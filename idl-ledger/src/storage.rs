use crate::{Decision, Evidence, LedgerIndex, Question, Result, LedgerError};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;

pub struct LedgerStorage {
    root: PathBuf,
}

impl LedgerStorage {
    pub fn new<P: AsRef<Path>>(root: P) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(self.root.join("questions"))?;
        fs::create_dir_all(self.root.join("decisions"))?;
        fs::create_dir_all(self.root.join("evidence"))?;
        
        let index = LedgerIndex::new();
        self.save_index(&index)?;
        
        Ok(())
    }

    pub fn save_question(&self, question: &Question) -> Result<()> {
        let path = self.root.join("questions").join(format!("{}.json", question.id));
        let json = serde_json::to_string_pretty(question)?;
        fs::write(path, json)?;
        self.update_index()?;
        Ok(())
    }

    pub fn save_decision(&self, decision: &Decision) -> Result<()> {
        let path = self.root.join("decisions").join(format!("{}.json", decision.id));
        let json = serde_json::to_string_pretty(decision)?;
        fs::write(path, json)?;
        self.update_index()?;
        Ok(())
    }

    pub fn save_evidence(&self, evidence: &Evidence) -> Result<()> {
        let path = self.root.join("evidence").join(format!("{}.json", evidence.id));
        let json = serde_json::to_string_pretty(evidence)?;
        fs::write(path, json)?;
        self.update_index()?;
        Ok(())
    }

    pub fn load_question(&self, id: Uuid) -> Result<Question> {
        let path = self.root.join("questions").join(format!("{}.json", id));
        let json = fs::read_to_string(&path)
            .map_err(|_| LedgerError::NotFound(format!("Question {}", id)))?;
        Ok(serde_json::from_str(&json)?)
    }

    pub fn load_decision(&self, id: Uuid) -> Result<Decision> {
        let path = self.root.join("decisions").join(format!("{}.json", id));
        let json = fs::read_to_string(&path)
            .map_err(|_| LedgerError::NotFound(format!("Decision {}", id)))?;
        Ok(serde_json::from_str(&json)?)
    }

    pub fn load_evidence(&self, id: Uuid) -> Result<Evidence> {
        let path = self.root.join("evidence").join(format!("{}.json", id));
        let json = fs::read_to_string(&path)
            .map_err(|_| LedgerError::NotFound(format!("Evidence {}", id)))?;
        Ok(serde_json::from_str(&json)?)
    }

    pub fn list_questions(&self) -> Result<Vec<Question>> {
        self.load_entries("questions")
    }

    pub fn list_decisions(&self) -> Result<Vec<Decision>> {
        self.load_entries("decisions")
    }

    pub fn list_evidence(&self) -> Result<Vec<Evidence>> {
        self.load_entries("evidence")
    }

    fn load_entries<T: serde::de::DeserializeOwned>(&self, subdir: &str) -> Result<Vec<T>> {
        let dir = self.root.join(subdir);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = Vec::new();
        for entry in WalkDir::new(dir).max_depth(1) {
            let entry = entry?;
            if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                let json = fs::read_to_string(entry.path())?;
                if let Ok(item) = serde_json::from_str(&json) {
                    entries.push(item);
                }
            }
        }
        Ok(entries)
    }

    pub fn load_index(&self) -> Result<LedgerIndex> {
        let path = self.root.join("index.json");
        if !path.exists() {
            return Ok(LedgerIndex::new());
        }
        let json = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&json)?)
    }

    pub fn save_index(&self, index: &LedgerIndex) -> Result<()> {
        let path = self.root.join("index.json");
        let json = serde_json::to_string_pretty(index)?;
        fs::write(path, json)?;
        Ok(())
    }

    fn update_index(&self) -> Result<()> {
        let mut index = self.load_index()?;
        index.update_counts(
            self.list_questions()?.len(),
            self.list_decisions()?.len(),
            self.list_evidence()?.len(),
        );
        self.save_index(&index)
    }
}
