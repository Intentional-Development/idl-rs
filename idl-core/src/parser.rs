use crate::ast::*;
use crate::error::{IdlError, Result};
use std::collections::HashMap;

/// IDL Parser - Initial implementation supporting core block types
pub struct IdlParser {
    input: String,
    position: usize,
    line: usize,
    column: usize,
}

impl IdlParser {
    pub fn new(input: String) -> Self {
        Self {
            input,
            position: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn parse(&mut self) -> Result<IdlDocument> {
        let mut imports = Vec::new();
        let mut blocks = Vec::new();
        let mut version = String::from("0.10.0");
        let mut module = None;
        let mut metadata = DocumentMetadata {
            version: None,
            source: None,
            lifecycle: None,
            drift_policy: None,
            trace_policy: None,
        };

        self.skip_whitespace_and_comments();

        // Parse metadata section
        while !self.is_at_end() {
            let keyword = self.peek_keyword();

            match keyword.as_str() {
                "idl_version" => {
                    self.consume_keyword("idl_version")?;
                    self.skip_whitespace();
                    self.consume_char(':')?;
                    self.skip_whitespace();
                    version = self.parse_string_literal()?;
                    self.skip_whitespace_and_comments();
                }
                "module" => {
                    self.consume_keyword("module")?;
                    self.skip_whitespace();
                    module = Some(self.parse_module()?);
                    self.skip_whitespace_and_comments();
                }
                "import" => {
                    self.consume_keyword("import")?;
                    self.skip_whitespace();
                    imports.push(self.parse_string_literal()?);
                    self.skip_whitespace_and_comments();
                }
                "version" => {
                    self.consume_keyword("version")?;
                    self.skip_whitespace();
                    metadata.version = Some(self.parse_string_literal()?);
                    self.skip_whitespace_and_comments();
                }
                "source" => {
                    self.consume_keyword("source")?;
                    self.skip_whitespace();
                    let val = self.parse_identifier()?;
                    metadata.source = Some(match val.as_str() {
                        "spec" => SourceType::Spec,
                        "code" => SourceType::Code,
                        _ => {
                            return Err(IdlError::ParseError {
                                line: self.line,
                                column: self.column,
                                message: format!("Invalid source type: {}", val),
                            })
                        }
                    });
                    self.skip_whitespace_and_comments();
                }
                "lifecycle" => {
                    self.consume_keyword("lifecycle")?;
                    self.skip_whitespace();
                    let val = self.parse_identifier()?;
                    metadata.lifecycle = Some(match val.as_str() {
                        "managed" => LifecycleType::Managed,
                        "exploratory" => LifecycleType::Exploratory,
                        "archived" => LifecycleType::Archived,
                        _ => {
                            return Err(IdlError::ParseError {
                                line: self.line,
                                column: self.column,
                                message: format!("Invalid lifecycle type: {}", val),
                            })
                        }
                    });
                    self.skip_whitespace_and_comments();
                }
                "drift" => {
                    self.consume_keyword("drift")?;
                    self.skip_whitespace();
                    self.consume_keyword("policy")?;
                    self.skip_whitespace();
                    let val = self.parse_identifier()?;
                    metadata.drift_policy = Some(match val.as_str() {
                        "fail" => DriftPolicy::Fail,
                        "warn" => DriftPolicy::Warn,
                        "ignore" => DriftPolicy::Ignore,
                        _ => {
                            return Err(IdlError::ParseError {
                                line: self.line,
                                column: self.column,
                                message: format!("Invalid drift policy: {}", val),
                            })
                        }
                    });
                    self.skip_whitespace_and_comments();
                }
                "trace" => {
                    self.consume_keyword("trace")?;
                    self.skip_whitespace();
                    self.consume_keyword("policy")?;
                    self.skip_whitespace();
                    let val = self.parse_identifier()?;
                    metadata.trace_policy = Some(match val.as_str() {
                        "strict" => TracePolicy::Strict,
                        "advisory" => TracePolicy::Advisory,
                        "none" => TracePolicy::None,
                        _ => {
                            return Err(IdlError::ParseError {
                                line: self.line,
                                column: self.column,
                                message: format!("Invalid trace policy: {}", val),
                            })
                        }
                    });
                    self.skip_whitespace_and_comments();
                }
                _ => break, // Start of blocks
            }
        }

        // Parse blocks
        while !self.is_at_end() {
            self.skip_whitespace_and_comments();
            if self.is_at_end() {
                break;
            }
            blocks.push(self.parse_block()?);
            self.skip_whitespace_and_comments();
        }

        Ok(IdlDocument {
            version,
            module,
            imports,
            blocks,
            metadata,
        })
    }

    fn parse_module(&mut self) -> Result<Module> {
        let full_name = self.parse_dotted_identifier()?;
        let path: Vec<String> = full_name.split('.').map(|s| s.to_string()).collect();
        Ok(Module {
            name: full_name,
            path,
        })
    }

    fn parse_block(&mut self) -> Result<Block> {
        let block_type = self.peek_keyword();

        match block_type.as_str() {
            "intent" => Ok(Block::Intent(self.parse_intent()?)),
            "scope" => Ok(Block::Scope(self.parse_scope()?)),
            "entity" => Ok(Block::Entity(self.parse_entity()?)),
            "event" => Ok(Block::Event(self.parse_event()?)),
            "rule" => Ok(Block::Rule(self.parse_rule()?)),
            "invariant" => Ok(Block::Invariant(self.parse_invariant()?)),
            "api" => Ok(Block::Api(self.parse_api()?)),
            _ => {
                // For now, treat unknown blocks as extensions
                self.parse_extension_block()
            }
        }
    }

    fn parse_intent(&mut self) -> Result<IntentBlock> {
        self.consume_keyword("intent")?;
        self.skip_whitespace();
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        self.consume_char('{')?;

        let fields = self.parse_key_value_block()?;

        Ok(IntentBlock {
            name,
            goal: fields
                .get("goal")
                .ok_or_else(|| IdlError::MissingRequiredField("goal".to_string()))?
                .clone(),
            outcome: fields.get("outcome").cloned(),
            actors: self.parse_array_field(&fields, "actors"),
            business_value: fields.get("business_value").cloned(),
            priority: fields.get("priority").and_then(|p| match p.as_str() {
                "high" => Some(Priority::High),
                "medium" => Some(Priority::Medium),
                "low" => Some(Priority::Low),
                _ => None,
            }),
        })
    }

    fn parse_scope(&mut self) -> Result<ScopeBlock> {
        self.consume_keyword("scope")?;
        self.skip_whitespace();
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        self.consume_char('{')?;

        let fields = self.parse_key_value_block()?;

        Ok(ScopeBlock {
            name,
            includes: self.parse_array_field(&fields, "includes"),
            excludes: self.parse_array_field(&fields, "excludes"),
        })
    }

    fn parse_entity(&mut self) -> Result<EntityBlock> {
        self.consume_keyword("entity")?;
        self.skip_whitespace();
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        self.consume_char('{')?;

        let fields = self.parse_key_value_block()?;

        Ok(EntityBlock {
            name,
            description: fields.get("description").cloned(),
            properties: HashMap::new(), // TODO: Parse properties block
            invariants: self.parse_array_field(&fields, "invariants"),
            storage: None,
            access_patterns: Vec::new(),
        })
    }

    fn parse_event(&mut self) -> Result<EventBlock> {
        self.consume_keyword("event")?;
        self.skip_whitespace();
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        self.consume_char('{')?;

        let _fields = self.parse_key_value_block()?;

        Ok(EventBlock {
            name,
            payload: HashMap::new(), // TODO: Parse payload block
            source: None,
        })
    }

    fn parse_rule(&mut self) -> Result<RuleBlock> {
        self.consume_keyword("rule")?;
        self.skip_whitespace();
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        self.consume_char('{')?;

        let fields = self.parse_key_value_block()?;

        Ok(RuleBlock {
            name,
            when: fields
                .get("when")
                .ok_or_else(|| IdlError::MissingRequiredField("when".to_string()))?
                .clone(),
            then: fields
                .get("then")
                .ok_or_else(|| IdlError::MissingRequiredField("then".to_string()))?
                .clone(),
            category: fields.get("category").and_then(|c| match c.as_str() {
                "behavioral" => Some(RuleCategory::Behavioral),
                "temporal" => Some(RuleCategory::Temporal),
                "conditional" => Some(RuleCategory::Conditional),
                _ => None,
            }),
        })
    }

    fn parse_invariant(&mut self) -> Result<InvariantBlock> {
        self.consume_keyword("invariant")?;
        self.skip_whitespace();
        let name = self.parse_string_literal()?;
        self.skip_whitespace();
        self.consume_char('{')?;

        let fields = self.parse_key_value_block()?;
        let expression = fields
            .get("expression")
            .or_else(|| fields.keys().next().map(|k| fields.get(k).unwrap()))
            .cloned()
            .unwrap_or_default();

        Ok(InvariantBlock {
            name,
            expression,
            scope: fields.get("scope").cloned(),
        })
    }

    fn parse_api(&mut self) -> Result<ApiBlock> {
        self.consume_keyword("api")?;
        self.skip_whitespace();
        let name = self.parse_identifier()?;
        self.skip_whitespace();
        self.consume_char('{')?;

        let fields = self.parse_key_value_block()?;

        Ok(ApiBlock {
            name,
            description: fields.get("description").cloned(),
            base_path: fields
                .get("base_path")
                .cloned()
                .unwrap_or_else(|| "/".to_string()),
            endpoints: Vec::new(), // TODO: Parse endpoint blocks
        })
    }

    fn parse_extension_block(&mut self) -> Result<Block> {
        let block_type = self.parse_identifier()?;
        self.skip_whitespace();
        let name = if self.current_char() == Some('{') {
            None
        } else {
            Some(self.parse_identifier()?)
        };
        self.skip_whitespace();
        self.consume_char('{')?;

        let fields_raw = self.parse_key_value_block()?;
        let mut fields = HashMap::new();
        for (k, v) in fields_raw {
            fields.insert(k, serde_json::Value::String(v));
        }

        Ok(Block::Extension(ExtensionBlock {
            block_type,
            name,
            fields,
        }))
    }

    fn parse_key_value_block(&mut self) -> Result<HashMap<String, String>> {
        let mut fields = HashMap::new();
        self.skip_whitespace_and_comments();

        while self.current_char() != Some('}') {
            self.skip_whitespace_and_comments();
            if self.current_char() == Some('}') {
                break;
            }

            let key = self.parse_identifier()?;
            self.skip_whitespace();
            self.consume_char(':')?;
            self.skip_whitespace();
            let value = self.parse_value()?;
            fields.insert(key, value);
            self.skip_whitespace_and_comments();
        }

        self.consume_char('}')?;
        Ok(fields)
    }

    fn parse_value(&mut self) -> Result<String> {
        self.skip_whitespace();
        let ch = self.current_char();

        match ch {
            Some('"') => self.parse_string_literal(),
            Some('[') => self.parse_array_value(),
            Some('{') => self.parse_nested_block(),
            _ => self.parse_identifier(),
        }
    }

    fn parse_array_value(&mut self) -> Result<String> {
        self.consume_char('[')?;
        let mut items = Vec::new();
        self.skip_whitespace_and_comments();

        while self.current_char() != Some(']') {
            self.skip_whitespace_and_comments();
            if self.current_char() == Some(']') {
                break;
            }
            items.push(self.parse_value()?);
            self.skip_whitespace_and_comments();
            if self.current_char() == Some(',') {
                self.advance();
            }
        }

        self.consume_char(']')?;
        Ok(format!("[{}]", items.join(", ")))
    }

    fn parse_nested_block(&mut self) -> Result<String> {
        self.consume_char('{')?;
        let mut content = String::from("{");
        let mut depth = 1;

        while depth > 0 && !self.is_at_end() {
            let ch = self.current_char().unwrap();
            content.push(ch);
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
            }
            self.advance();
        }

        Ok(content)
    }

    fn parse_array_field(&self, fields: &HashMap<String, String>, key: &str) -> Vec<String> {
        fields
            .get(key)
            .map(|v| {
                if v.starts_with('[') && v.ends_with(']') {
                    v[1..v.len() - 1]
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                } else {
                    vec![v.clone()]
                }
            })
            .unwrap_or_default()
    }

    fn parse_string_literal(&mut self) -> Result<String> {
        self.consume_char('"')?;
        let mut value = String::new();

        while let Some(ch) = self.current_char() {
            if ch == '"' {
                self.advance();
                return Ok(value);
            }
            if ch == '\\' {
                self.advance();
                if let Some(escaped) = self.current_char() {
                    value.push(match escaped {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '"' => '"',
                        '\\' => '\\',
                        _ => escaped,
                    });
                    self.advance();
                }
            } else {
                value.push(ch);
                self.advance();
            }
        }

        Err(IdlError::ParseError {
            line: self.line,
            column: self.column,
            message: "Unterminated string literal".to_string(),
        })
    }

    fn parse_identifier(&mut self) -> Result<String> {
        let mut ident = String::new();

        while let Some(ch) = self.current_char() {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if ident.is_empty() {
            return Err(IdlError::ParseError {
                line: self.line,
                column: self.column,
                message: "Expected identifier".to_string(),
            });
        }

        Ok(ident)
    }

    fn parse_dotted_identifier(&mut self) -> Result<String> {
        let mut ident = String::new();

        while let Some(ch) = self.current_char() {
            if ch.is_alphanumeric() || ch == '_' || ch == '.' {
                ident.push(ch);
                self.advance();
            } else {
                break;
            }
        }

        if ident.is_empty() {
            return Err(IdlError::ParseError {
                line: self.line,
                column: self.column,
                message: "Expected dotted identifier".to_string(),
            });
        }

        Ok(ident)
    }

    fn peek_keyword(&self) -> String {
        let mut pos = self.position;
        let mut keyword = String::new();

        while pos < self.input.len() {
            let ch = self.input.chars().nth(pos).unwrap();
            if ch.is_alphabetic() || ch == '_' || ch == '-' {
                keyword.push(ch);
                pos += 1;
            } else {
                break;
            }
        }

        keyword
    }

    fn consume_keyword(&mut self, keyword: &str) -> Result<()> {
        let actual = self.parse_identifier()?;
        if actual != keyword {
            return Err(IdlError::ParseError {
                line: self.line,
                column: self.column,
                message: format!("Expected keyword '{}', got '{}'", keyword, actual),
            });
        }
        Ok(())
    }

    fn consume_char(&mut self, expected: char) -> Result<()> {
        let ch = self.current_char().ok_or_else(|| IdlError::ParseError {
            line: self.line,
            column: self.column,
            message: format!("Expected '{}', got end of file", expected),
        })?;

        if ch != expected {
            return Err(IdlError::ParseError {
                line: self.line,
                column: self.column,
                message: format!("Expected '{}', got '{}'", expected, ch),
            });
        }

        self.advance();
        Ok(())
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current_char() {
            if ch.is_whitespace() {
                if ch == '\n' {
                    self.line += 1;
                    self.column = 1;
                } else {
                    self.column += 1;
                }
                self.position += 1;
            } else {
                break;
            }
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            self.skip_whitespace();
            if self.current_char() == Some('/') && self.peek_char() == Some('/') {
                // Line comment
                while let Some(ch) = self.current_char() {
                    if ch == '\n' {
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn current_char(&self) -> Option<char> {
        self.input.chars().nth(self.position)
    }

    fn peek_char(&self) -> Option<char> {
        self.input.chars().nth(self.position + 1)
    }

    fn advance(&mut self) {
        if let Some(ch) = self.current_char() {
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            self.position += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }
}

pub fn parse_idl(input: &str) -> Result<IdlDocument> {
    let mut parser = IdlParser::new(input.to_string());
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_document() {
        let input = r#"
idl_version: "0.10.0"

module Test.Example

intent TestIntent {
  goal: "Test goal"
}
"#;
        let result = parse_idl(input);
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(doc.version, "0.10.0");
        assert_eq!(doc.blocks.len(), 1);
    }

    #[test]
    fn test_parse_entity() {
        let input = r#"
idl_version: "0.10.0"

entity User {
  description: "A user in the system"
}
"#;
        let result = parse_idl(input);
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert_eq!(doc.blocks.len(), 1);
    }
}
