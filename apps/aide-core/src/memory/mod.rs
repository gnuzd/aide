use anyhow::{Result, anyhow};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use fastembed::{TextEmbedding, InitOptions, EmbeddingModel};

pub struct MemoryStore {
    conn: Connection,
    embedding_model: TextEmbedding,
}

fn now_iso8601() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let (year, month, day) = days_to_ymd(secs / 86400);
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hours, minutes, seconds)
}

fn days_to_ymd(mut days: u64) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let days_in_year = if is_leap_year(year) { 366u64 } else { 365u64 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap_year(year);
    let month_days: [u64; 12] = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, (days + 1) as u32)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub fn generate_session_id() -> String {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("sess_{:x}_{:x}", duration.as_secs(), duration.subsec_nanos())
}

// (pattern to detect in lowercased message, canonical stored name)
const LANG_PATTERNS: &[(&str, &str)] = &[
    ("rust", "rust"),
    ("python", "python"),
    ("golang", "go"),
    (" go ", "go"),
    ("javascript", "javascript"),
    (" js ", "javascript"),
    ("typescript", "typescript"),
    (" ts ", "typescript"),
    ("c++", "c++"),
    ("java", "java"),
    ("ruby", "ruby"),
    ("swift", "swift"),
    ("kotlin", "kotlin"),
    ("c#", "c#"),
    ("csharp", "c#"),
    ("php", "php"),
    ("scala", "scala"),
    ("haskell", "haskell"),
    ("erlang", "erlang"),
    ("elixir", "elixir"),
    ("lua", "lua"),
    ("bash", "bash"),
    ("shell script", "shell"),
];

const TOPIC_PATTERNS: &[(&str, &str)] = &[
    ("web development", "web"),
    ("web app", "web"),
    ("frontend", "frontend"),
    ("backend", "backend"),
    ("machine learning", "ml"),
    (" ml ", "ml"),
    ("deep learning", "ml"),
    ("devops", "devops"),
    ("embedded", "embedded"),
    (" cli ", "cli"),
    ("command line", "cli"),
    ("systems programming", "systems"),
    ("database", "database"),
    (" api ", "api"),
    ("rest api", "api"),
    ("mobile app", "mobile"),
    ("game dev", "game development"),
    ("cloud", "cloud"),
];

const BEGINNER_SIGNALS: &[&str] = &[
    "i'm new to",
    "im new to",
    "how do i",
    "i'm learning",
    "im learning",
    "newbie",
    "beginner",
    "just started",
];

const EXPERT_SIGNALS: &[&str] = &[
    "i've built",
    "ive built",
    "in production",
    "years of experience",
    "senior engineer",
    "i've been using",
    "ive been using",
    "at scale",
];

const PROJECT_SIGNALS: &[&str] = &[
    "my project",
    "i'm building",
    "im building",
    "working on",
    "i'm working on",
    "im working on",
];

impl MemoryStore {
    pub fn init_db(base_path: &PathBuf) -> Result<Self> {
        std::fs::create_dir_all(base_path)?;
        let db_path = base_path.join("memory.db");
        let conn = Connection::open(&db_path)?;

        let embedding_model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2)
        ).map_err(|e| anyhow!("Embedding model init failed: {}", e))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS conversations (
                id                 INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id         TEXT    NOT NULL,
                turn_number        INTEGER NOT NULL,
                user_message       TEXT    NOT NULL,
                assistant_response TEXT    NOT NULL,
                timestamp          TEXT    NOT NULL
            );

            CREATE TABLE IF NOT EXISTS user_profile (
                key          TEXT PRIMARY KEY,
                value        TEXT NOT NULL,
                last_updated TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS long_term_memory (
                id INTEGER PRIMARY KEY,
                content TEXT NOT NULL,
                vector BLOB NOT NULL,
                timestamp TEXT NOT NULL
            );
        ")?;

        Ok(Self { conn, embedding_model })
    }

    pub fn store_semantic(&self, text: &str) -> Result<()> {
        if text.trim().is_empty() { return Ok(()); }
        let embeddings = self.embedding_model.embed(vec![text], None)
            .map_err(|e| anyhow!("Embedding failed: {}", e))?;
        let vector_blob = bincode::serialize(&embeddings[0])?;

        self.conn.execute(
            "INSERT INTO long_term_memory (content, vector, timestamp) VALUES (?1, ?2, ?3)",
            params![text, vector_blob, now_iso8601()],
        )?;
        Ok(())
    }

    pub fn search_semantic(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        if query.trim().is_empty() { return Ok(vec![]); }
        let query_vec = self.embedding_model.embed(vec![query], None)
            .map_err(|e| anyhow!("Query embedding failed: {}", e))?[0].clone();

        let mut stmt = self.conn.prepare("SELECT content, vector FROM long_term_memory")?;
        let rows = stmt.query_map([], |row| {
            let content: String = row.get(0)?;
            let vec_blob: Vec<u8> = row.get(1)?;
            let vec: Vec<f32> = bincode::deserialize(&vec_blob).unwrap_or_default();
            Ok((content, vec))
        })?;

        let mut results = Vec::new();
        for row in rows {
            if let Ok((content, vec)) = row {
                let score = cosine_similarity(&query_vec, &vec);
                if score > 0.4 { // Threshold for relevance
                    results.push((score, content));
                }
            }
        }

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(results.into_iter().take(limit).map(|(_, c)| c).collect())
    }

    pub fn save_turn(&self, session_id: &str, turn: u32, user_msg: &str, response: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO conversations (session_id, turn_number, user_message, assistant_response, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, turn, user_msg, response, now_iso8601()],
        )?;

        // Automatically store this turn in semantic memory for long-term recall
        // We store the user's input paired with Aide's response to provide context
        let semantic_entry = format!("User: {}\nAide: {}", user_msg, response);
        let _ = self.store_semantic(&semantic_entry);

        Ok(())
    }

    pub fn extract_and_learn(&self, user_message: &str, prev_assistant_msg: Option<&str>) -> Result<()> {
        let user_trim = user_message.trim();
        // Pad with spaces so patterns like " go " match at start/end of message too
        let lower = format!(" {} ", user_trim.to_lowercase());

        // Contextual auto-memory: if Aide asked a question, the user's response is an important fact.
        if let Some(prev) = prev_assistant_msg {
            let prev_trim = prev.trim();
            // Check if previous message contained a question mark, not just ended with one
            if prev_trim.contains('?') {
                // This is likely an answer to a question. Store it semantically.
                let contextual_fact = format!("Aide asked: {}\nUser answered: {}", prev_trim, user_trim);
                let _ = self.store_semantic(&contextual_fact);
            }
        }

        // Self-introduction patterns
        let intro_patterns = [
            "my name is ",
            "i am ",
            "i'm ",
            "call me ",
            "everyone calls me ",
            "i live in ",
            "i'm from ",
            "im from ",
            "my favorite ",
            "i like ",
            "i love ",
            "i hate ",
            "i don't like ",
            "dont like ",
            "my hobby is ",
            "my work is ",
            "i work at ",
            "i work as ",
        ];
        for intro in intro_patterns {
            if lower.contains(&format!(" {} ", intro)) || lower.trim().starts_with(intro) {
                // Store the whole sentence or just the fact?
                // Let's store the whole message as a potential identity fact if it's short
                if user_trim.len() < 100 {
                    let _ = self.store_semantic(&format!("User identity/intro: {}", user_trim));
                }
            }
        }

        // Languages
        let mut found_langs: Vec<&str> = Vec::new();
        for &(pattern, canonical) in LANG_PATTERNS {
            if lower.contains(pattern) && !found_langs.contains(&canonical) {
                found_langs.push(canonical);
            }
        }
        if !found_langs.is_empty() {
            let existing = self.get_profile_value("languages_mentioned").unwrap_or_default();
            let mut set: Vec<String> = if existing.is_empty() {
                Vec::new()
            } else {
                existing.split(',').map(|s| s.trim().to_string()).collect()
            };
            for &lang in &found_langs {
                let l = lang.to_string();
                if !set.contains(&l) {
                    set.push(l);
                }
            }
            self.upsert_profile("languages_mentioned", &set.join(","))?;
        }

        // Skill level (last-write-wins)
        let is_expert = EXPERT_SIGNALS.iter().any(|&s| lower.contains(s));
        let is_beginner = BEGINNER_SIGNALS.iter().any(|&s| lower.contains(s));
        if is_expert {
            self.upsert_profile("skill_level", "expert")?;
        } else if is_beginner {
            self.upsert_profile("skill_level", "beginner")?;
        }

        // Topics of interest
        let mut found_topics: Vec<&str> = Vec::new();
        for &(pattern, canonical) in TOPIC_PATTERNS {
            if lower.contains(pattern) && !found_topics.contains(&canonical) {
                found_topics.push(canonical);
            }
        }
        if !found_topics.is_empty() {
            let existing = self.get_profile_value("topics_of_interest").unwrap_or_default();
            let mut set: Vec<String> = if existing.is_empty() {
                Vec::new()
            } else {
                existing.split(',').map(|s| s.trim().to_string()).collect()
            };
            for &topic in &found_topics {
                let t = topic.to_string();
                if !set.contains(&t) {
                    set.push(t);
                }
            }
            self.upsert_profile("topics_of_interest", &set.join(","))?;
        }

        // Active project flag
        if PROJECT_SIGNALS.iter().any(|&s| lower.contains(s)) {
            self.upsert_profile("has_active_project", "true")?;
        }

        // Explicit "remember X" / "don't forget X" trigger
        let lower_trim = user_message.trim().to_lowercase();
        const REMEMBER_TRIGGERS: &[&str] = &[
            "please remember that ",
            "please remember my ",
            "please remember i ",
            "please remember ",
            "remember that ",
            "remember my ",
            "remember i ",
            "remember: ",
            "remember ",
            "don't forget that ",
            "don't forget ",
            "dont forget that ",
            "dont forget ",
        ];
        for trigger in REMEMBER_TRIGGERS {
            if lower_trim.starts_with(trigger) {
                let fact = user_message.trim()[trigger.len()..].trim();
                if !fact.is_empty() {
                    let existing = self.get_profile_value("remembered_facts").unwrap_or_default();
                    let fact_lower = fact.to_lowercase();
                    let already_stored = existing
                        .split('|')
                        .any(|f| f.trim().to_lowercase() == fact_lower);
                    if !already_stored {
                        let new_val = if existing.is_empty() {
                            fact.to_string()
                        } else {
                            format!("{}|{}", existing, fact)
                        };
                        self.upsert_profile("remembered_facts", &new_val)?;
                    }
                    // Trigger semantic storage
                    let _ = self.store_semantic(fact);
                }
                break;
            }
        }

        // Increment total turns
        self.increment_counter("total_turns")?;

        Ok(())
    }

    pub fn get_profile_summary(&self) -> Result<String> {
        let base = "You are Aide, a helpful assistant. \
When the user specifically asks you to create, generate, or produce an image, first describe what you're creating, \
then return a fenced code block with language 'image-prompt' containing a detailed, high-quality prompt for Stable Diffusion. \
Otherwise, chat normally and do not output image prompts. Do not output base64 data, and do not output Python or shell commands for image generation. \
IMPORTANT: If you don't know something about the user (like their name, preference, or context) and it's relevant to the conversation, \
feel free to ask them. You have long-term memory and will remember their answers for future sessions.";

        let languages = self.get_profile_value("languages_mentioned").unwrap_or_default();
        let skill = self.get_profile_value("skill_level").unwrap_or_default();
        let topics = self.get_profile_value("topics_of_interest").unwrap_or_default();
        let has_project = self.get_profile_value("has_active_project").unwrap_or_default();
        let total_turns = self.get_profile_value("total_turns").unwrap_or_default();
        let remembered = self.get_profile_value("remembered_facts").unwrap_or_default();

        if languages.is_empty() && skill.is_empty() && topics.is_empty()
            && total_turns.is_empty() && remembered.is_empty()
        {
            return Ok(base.to_string());
        }

        let mut parts = vec![base.to_string()];

        if !languages.is_empty() {
            let langs_display: Vec<String> = languages
                .split(',')
                .map(|l| capitalize(l.trim()))
                .collect();
            parts.push(format!("The user works with: {}.", langs_display.join(", ")));
        }

        if !skill.is_empty() {
            parts.push(format!("Their experience level appears to be {}.", skill));
        }

        if !topics.is_empty() {
            let topics_display: Vec<String> = topics.split(',').map(|t| t.trim().to_string()).collect();
            parts.push(format!("They are interested in: {}.", topics_display.join(", ")));
        }

        if !total_turns.is_empty() && total_turns != "0" {
            parts.push(format!("You have had {} conversations together.", total_turns));
        }

        if has_project == "true" {
            parts.push("The user is actively building a project.".to_string());
        }

        if !remembered.is_empty() {
            let facts: Vec<&str> = remembered.split('|').map(|f| f.trim()).collect();
            parts.push(format!("The user has asked you to remember: {}.", facts.join("; ")));
        }

        Ok(parts.join(" "))
    }

    pub fn conversation_stats(&self) -> Result<(i64, i64)> {
        let turns: i64 = self.conn
            .query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
            .unwrap_or(0);
        let sessions: i64 = self.conn
            .query_row("SELECT COUNT(DISTINCT session_id) FROM conversations", [], |r| r.get(0))
            .unwrap_or(0);
        Ok((turns, sessions))
    }

    pub fn profile_entry_count(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM user_profile", [], |r| r.get(0))
            .unwrap_or(0)
    }

    pub fn remembered_facts_count(&self) -> usize {
        self.get_profile_value("remembered_facts")
            .map(|v| v.split('|').filter(|s| !s.trim().is_empty()).count())
            .unwrap_or(0)
    }

    pub fn semantic_facts_count(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM long_term_memory", [], |r| r.get(0))
            .unwrap_or(0)
    }

    pub fn clear_conversations(&self) -> Result<()> {
        self.conn.execute("DELETE FROM conversations", [])?;
        self.conn.execute("DELETE FROM user_profile WHERE key = 'total_turns'", [])?;
        Ok(())
    }

    pub fn clear_profile(&self) -> Result<()> {
        self.conn.execute("DELETE FROM user_profile", [])?;
        self.conn.execute("DELETE FROM long_term_memory", [])?;
        Ok(())
    }

    pub fn clear_remembered_facts(&self) -> Result<()> {
        self.conn.execute("DELETE FROM user_profile WHERE key = 'remembered_facts'", [])?;
        Ok(())
    }

    pub fn load_recent_history(&self, limit: usize) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT user_message, assistant_response FROM conversations ORDER BY id DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;

        let mut history = Vec::new();
        for row in rows {
            history.push(row?);
        }
        history.reverse();
        Ok(history)
    }

    fn get_profile_value(&self, key: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT value FROM user_profile WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .ok()
    }

    fn upsert_profile(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO user_profile (key, value, last_updated) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, last_updated = excluded.last_updated",
            params![key, value, now_iso8601()],
        )?;
        Ok(())
    }

    fn increment_counter(&self, key: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO user_profile (key, value, last_updated) VALUES (?1, '1', ?2)
             ON CONFLICT(key) DO UPDATE SET value = CAST(CAST(value AS INTEGER) + 1 AS TEXT), last_updated = excluded.last_updated",
            params![key, now_iso8601()],
        )?;
        Ok(())
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
    let dot: f32 = v1.iter().zip(v2).map(|(a, b)| a * b).sum();
    let n1: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
    let n2: f32 = v2.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n1 == 0.0 || n2 == 0.0 { return 0.0; }
    dot / (n1 * n2)
}
