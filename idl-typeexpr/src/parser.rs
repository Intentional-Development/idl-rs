//! Hand-written recursive descent parser for TypeExpr (W17).

use crate::types::{PrimitiveType, TypeExpr};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq)]
pub enum ParseError {
    #[error("unexpected end of input at position {0}")]
    UnexpectedEof(usize),
    #[error("expected {expected} at position {pos}, found '{found}'")]
    Expected {
        expected: String,
        found: String,
        pos: usize,
    },
    #[error("invalid reference name at position {0}")]
    InvalidReference(usize),
    #[error("empty union at position {0}")]
    EmptyUnion(usize),
}

pub struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn current(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn peek(&self, offset: usize) -> Option<char> {
        self.input[self.pos..].chars().nth(offset)
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.current() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    fn consume(&mut self, ch: char) -> Result<(), ParseError> {
        self.skip_whitespace();
        if self.current() == Some(ch) {
            self.pos += ch.len_utf8();
            Ok(())
        } else {
            Err(ParseError::Expected {
                expected: format!("'{}'", ch),
                found: self
                    .current()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "EOF".to_string()),
                pos: self.pos,
            })
        }
    }

    fn consume_str(&mut self, s: &str) -> Result<(), ParseError> {
        self.skip_whitespace();
        if self.input[self.pos..].starts_with(s) {
            self.pos += s.len();
            Ok(())
        } else {
            Err(ParseError::Expected {
                expected: format!("'{}'", s),
                found: self.input[self.pos..]
                    .chars()
                    .take(s.len())
                    .collect::<String>(),
                pos: self.pos,
            })
        }
    }

    fn parse_identifier(&mut self) -> Result<String, ParseError> {
        self.skip_whitespace();
        let start = self.pos;
        
        let first = self.current().ok_or(ParseError::UnexpectedEof(self.pos))?;
        if !first.is_ascii_uppercase() {
            return Err(ParseError::InvalidReference(self.pos));
        }
        
        self.pos += first.len_utf8();
        
        while let Some(ch) = self.current() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_keyword(&mut self, kw: &str) -> bool {
        self.skip_whitespace();
        if self.input[self.pos..].starts_with(kw) {
            let end = self.pos + kw.len();
            if let Some(ch) = self.input[end..].chars().next() {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    return false;
                }
            }
            self.pos = end;
            true
        } else {
            false
        }
    }

    pub fn parse(input: &str) -> Result<TypeExpr, ParseError> {
        let mut parser = Parser::new(input);
        let expr = parser.parse_union()?;
        parser.skip_whitespace();
        if parser.pos < parser.input.len() {
            return Err(ParseError::Expected {
                expected: "end of input".to_string(),
                found: parser.input[parser.pos..].chars().next().unwrap().to_string(),
                pos: parser.pos,
            });
        }
        Ok(expr)
    }

    fn parse_union(&mut self) -> Result<TypeExpr, ParseError> {
        let mut variants = vec![self.parse_nullable()?];
        
        loop {
            self.skip_whitespace();
            if self.current() == Some('|') {
                self.consume('|')?;
                variants.push(self.parse_nullable()?);
            } else {
                break;
            }
        }
        
        if variants.len() == 1 {
            Ok(variants.into_iter().next().unwrap())
        } else {
            Ok(TypeExpr::Union(variants))
        }
    }

    fn parse_nullable(&mut self) -> Result<TypeExpr, ParseError> {
        let inner = self.parse_array()?;
        
        self.skip_whitespace();
        if self.current() == Some('?') {
            self.consume('?')?;
            Ok(TypeExpr::Nullable(Box::new(inner)))
        } else {
            Ok(inner)
        }
    }

    fn parse_array(&mut self) -> Result<TypeExpr, ParseError> {
        let mut expr = self.parse_atom()?;
        
        loop {
            self.skip_whitespace();
            if self.current() == Some('[') && self.peek(1) == Some(']') {
                self.consume('[')?;
                self.consume(']')?;
                expr = TypeExpr::Array(Box::new(expr));
            } else {
                break;
            }
        }
        
        Ok(expr)
    }

    fn parse_atom(&mut self) -> Result<TypeExpr, ParseError> {
        self.skip_whitespace();
        
        if self.parse_keyword("Map") {
            return self.parse_map();
        }
        
        if self.current() == Some('(') {
            self.consume('(')?;
            self.skip_whitespace();
            
            if self.current() == Some(')') {
                self.consume(')')?;
                return Ok(TypeExpr::Unit);
            }
            
            let inner = self.parse_union()?;
            self.consume(')')?;
            return Ok(inner);
        }
        
        let word_start = self.pos;
        if let Some(ch) = self.current() {
            if ch.is_ascii_lowercase() {
                let mut word = String::new();
                while let Some(c) = self.current() {
                    if c.is_ascii_alphanumeric() || c == '_' {
                        word.push(c);
                        self.pos += c.len_utf8();
                    } else {
                        break;
                    }
                }
                
                if let Some(prim) = PrimitiveType::from_str(&word) {
                    return Ok(TypeExpr::Primitive(prim));
                } else {
                    return Err(ParseError::Expected {
                        expected: "primitive type or reference".to_string(),
                        found: word,
                        pos: word_start,
                    });
                }
            }
        }
        
        let name = self.parse_identifier()?;
        
        if name.contains('.') || self.current() == Some('.') {
            let mut full_name = name;
            while self.current() == Some('.') {
                self.consume('.')?;
                full_name.push('.');
                full_name.push_str(&self.parse_identifier()?);
            }
            Ok(TypeExpr::Reference(full_name))
        } else {
            Ok(TypeExpr::Reference(name))
        }
    }

    fn parse_map(&mut self) -> Result<TypeExpr, ParseError> {
        self.consume('<')?;
        let key = self.parse_union()?;
        self.consume(',')?;
        let value = self.parse_union()?;
        self.consume('>')?;
        
        Ok(TypeExpr::Map {
            key: Box::new(key),
            value: Box::new(value),
        })
    }
}

pub fn parse_type_expr(input: &str) -> Result<TypeExpr, ParseError> {
    Parser::parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_primitive() {
        assert_eq!(
            parse_type_expr("string"),
            Ok(TypeExpr::Primitive(PrimitiveType::String))
        );
        assert_eq!(
            parse_type_expr("integer"),
            Ok(TypeExpr::Primitive(PrimitiveType::Integer))
        );
    }

    #[test]
    fn test_parse_reference() {
        assert_eq!(
            parse_type_expr("User"),
            Ok(TypeExpr::Reference("User".to_string()))
        );
    }

    #[test]
    fn test_parse_array() {
        assert_eq!(
            parse_type_expr("string[]"),
            Ok(TypeExpr::Array(Box::new(TypeExpr::Primitive(
                PrimitiveType::String
            ))))
        );
        assert_eq!(
            parse_type_expr("User[]"),
            Ok(TypeExpr::Array(Box::new(TypeExpr::Reference(
                "User".to_string()
            ))))
        );
    }

    #[test]
    fn test_parse_nullable() {
        assert_eq!(
            parse_type_expr("string?"),
            Ok(TypeExpr::Nullable(Box::new(TypeExpr::Primitive(
                PrimitiveType::String
            ))))
        );
    }

    #[test]
    fn test_parse_union() {
        let result = parse_type_expr("string|integer").unwrap();
        match result {
            TypeExpr::Union(variants) => {
                assert_eq!(variants.len(), 2);
                assert_eq!(variants[0], TypeExpr::Primitive(PrimitiveType::String));
                assert_eq!(variants[1], TypeExpr::Primitive(PrimitiveType::Integer));
            }
            _ => panic!("expected union"),
        }
    }

    #[test]
    fn test_parse_map() {
        let result = parse_type_expr("Map<string, integer>").unwrap();
        match result {
            TypeExpr::Map { key, value } => {
                assert_eq!(*key, TypeExpr::Primitive(PrimitiveType::String));
                assert_eq!(*value, TypeExpr::Primitive(PrimitiveType::Integer));
            }
            _ => panic!("expected map"),
        }
    }

    #[test]
    fn test_parse_unit() {
        assert_eq!(parse_type_expr("()"), Ok(TypeExpr::Unit));
    }

    #[test]
    fn test_parse_complex() {
        assert_eq!(
            parse_type_expr("Map<string, User[]>?"),
            Ok(TypeExpr::Nullable(Box::new(TypeExpr::Map {
                key: Box::new(TypeExpr::Primitive(PrimitiveType::String)),
                value: Box::new(TypeExpr::Array(Box::new(TypeExpr::Reference(
                    "User".to_string()
                ))))
            })))
        );
    }
}
