# AI CLI Tool (Aide) - Project Plan

A local-first, intelligent CLI assistant built with Rust, featuring hardware-aware model management, SQLite-backed long-term memory, and autonomous task execution.

## 1. Core Architecture

The system will follow a modular architecture to ensure flexibility and local-first performance:

*   **CLI Layer (Clap):** Handles user input, commands, and interactive prompts.
*   **Orchestrator:** The "brain" that decides whether the current model can handle a request or if a specialized model is needed.
*   **Model Provider Interface:** A trait-based abstraction to support different backends (e.g., `llama.cpp` via `llama-cpp-rs`, `Wuerstchen`, or `Ollama` API).
*   **Memory Engine (SQLite + Vector Search):** Stores conversation history, user preferences, and learned "skills" using `rusqlite` and potentially a vector extension for RAG (Retrieval-Augmented Generation).
*   **Action Runner:** A sandbox-aware module to execute shell commands (git, file ops, etc.) with user confirmation.
*   **Hardware Profiler:** Detects CPU (AVX/NEON), GPU (Metal/CUDA/Vulkan), and RAM to recommend optimal GGUF quantization levels.

## 2. Technical Stack

*   **Language:** Rust
*   **CLI Framework:** `clap` (v4) with `ratatui` for rich UI elements if needed.
*   **Runtime:** `tokio` for async operations.
*   **AI Inference:** `llama-cpp-2` or `candle` (HuggingFace's ML framework in Rust) for local inference.
*   **Database:** `sqlite` (via `rusqlite`) for structured memory.
*   **Hardware Detection:** `sysinfo` and `raw-cpuid`.

## 3. Key Features & Implementation Steps

### Phase 1: Foundation & Hardware Profiling [COMPLETED]
1.  **System Audit:** Implemented modular hardware detection in `src/system/`.
    *   Checks RAM, CPU, and OS using `sysinfo`.
    *   Provides compatibility warnings if requirements aren't met.
2.  **Model Registry:** Defined a core set of models in `src/models/` with hardware mappings.
3.  **Setup Wizard:** Added a `setup` command to guide users through hardware profiling and model selection.

### Phase 2: Local Inference Engine [COMPLETED]
1.  **Model Downloader:** HuggingFace Hub integration via `reqwest` with streaming progress bars (`indicatif`). Downloads GGUF files to `~/.aide/models/`.
2.  **Inference Engine** (`src/models/inference.rs`): Local chat completion using `llama-cpp-2`.
    *   Multi-template prompt formatting: `llama3`, `phi3`, `deepseek`, `chatml`.
    *   Greedy sampling via direct logit argmax on `get_logits_ith()` — O(1) allocation per token.
    *   Streaming token output with `encoding_rs` UTF-8 decoding.
    *   Apple Silicon optimizations: Flash Attention (`LLAMA_FLASH_ATTN_TYPE_ENABLED`), full GPU offload (`n_gpu_layers = u32::MAX`), KV cache on Metal.
3.  **Chat Loop:** Interactive REPL via `rustyline` with history support.

## 4. Usage

To run the initial setup and audit your hardware:
```bash
cargo run --release -- setup
```

To start a chat session (recommend release build for best performance):
```bash
cargo run --release -- chat
```

To list available models:
```bash
cargo run --release -- models
```

To see raw system information:
```bash
cargo run --release -- system
```

### Phase 3: Memory & Learning (SQLite) [COMPLETED]
1.  **Schema Design** (`~/.aide/memory.db`, WAL mode):
    *   `conversations`: Full turn history (session_id, turn_number, user_message, assistant_response, timestamp).
    *   `user_profile`: Key/value profile store (languages_mentioned, skill_level, topics_of_interest, has_active_project, total_turns).
2.  **Learning Loop** (`src/memory/mod.rs`): In-process pattern matching per turn — no LLM call. Detects programming languages, skill level signals, topic interests, and active project indicators. Accumulated facts persist across sessions.
3.  **Personalized System Prompt:** Profile summary injected into the system prompt at session start (~60 tokens), personalizing responses based on accumulated knowledge. Each `aide chat` is a new session; old history is saved but not re-injected into context.

### Phase 4: Intent Classification & Model Switching [COMPLETED]
1.  **Router Logic:** LLM-based intent classification (CODING vs GENERAL) using a tiny `max_tokens=8` prompt against the active model — no external service needed.
2.  **Per-turn Switch Prompt:** If a better-suited model exists (or can be downloaded), the user is offered a session-only switch: *"Looks like a coding task — DeepSeek Coder handles it better. Download (~8 GB) and switch for this session? [y/N]"*. Declining or skipping keeps the current model. Each alternative is offered at most once per session (`already_offered` guard). No config file is modified — next `aide chat` loads the original model.

### Phase 5: Action Execution (Tools)
1.  **Function Calling:** Implement a parser for the model to output structured tool calls (e.g., `{"tool": "shell", "command": "git init"}`).
2.  **Safety Gate:** All destructive commands (rm, push, etc.) require explicit user `[y/N]` confirmation via the CLI.
3.  **Git Integration:** Pre-defined high-level actions for git workflows.

## 4. Proposed Database Schema (SQLite)

```sql
CREATE TABLE memory (
    id INTEGER PRIMARY KEY,
    key TEXT UNIQUE,
    value TEXT,
    last_updated DATETIME
);

CREATE TABLE chat_history (
    id INTEGER PRIMARY KEY,
    role TEXT, -- 'user' or 'assistant'
    content TEXT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE models (
    name TEXT PRIMARY KEY,
    path TEXT,
    quantization TEXT,
    is_main BOOLEAN
);
```

## 5. Development Roadmap

- **Week 1:** Project scaffolding, Hardware Audit module, CLI argument parsing.
- **Week 2:** Model downloading logic and `llama.cpp` integration.
- **Week 3:** SQLite memory implementation and persistent context.
- **Week 4:** Tool execution engine and intent routing.
- **Week 5:** Refinement, UI/UX improvements (spinners, markdown rendering in terminal).

## 6. Local-First Guarantees
- No telemetry by default.
- Data stays in `~/.local/share/aide` or equivalent.
- Works entirely offline once models are downloaded.
