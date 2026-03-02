use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct MemoryStore {
    conn: Connection,
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
        ")?;

        Ok(Self { conn })
    }

    pub fn save_turn(&self, session_id: &str, turn: u32, user_msg: &str, response: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO conversations (session_id, turn_number, user_message, assistant_response, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![session_id, turn, user_msg, response, now_iso8601()],
        )?;
        Ok(())
    }

    pub fn extract_and_learn(&self, user_message: &str) -> Result<()> {
        // Pad with spaces so patterns like " go " match at start/end of message too
        let lower = format!(" {} ", user_message.to_lowercase());

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
                }
                break;
            }
        }

        // Increment total turns
        self.increment_counter("total_turns")?;

        Ok(())
    }

    pub fn get_profile_summary(&self) -> Result<String> {
        let base = "You are Aide, a helpful assistant.";

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

    pub fn clear_conversations(&self) -> Result<()> {
        self.conn.execute("DELETE FROM conversations", [])?;
        self.conn.execute("DELETE FROM user_profile WHERE key = 'total_turns'", [])?;
        Ok(())
    }

    pub fn clear_profile(&self) -> Result<()> {
        self.conn.execute("DELETE FROM user_profile", [])?;
        Ok(())
    }

    pub fn clear_remembered_facts(&self) -> Result<()> {
        self.conn.execute("DELETE FROM user_profile WHERE key = 'remembered_facts'", [])?;
        Ok(())
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
