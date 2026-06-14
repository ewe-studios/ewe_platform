# Decision 09: Loop Detection

**Status:** Accepted  
**Date:** 2026-06-11  
**Context:** Specification 36 — Agentic API for foundation_ai

## Problem


LLMs can enter repetition loops — producing the same or near-identical content across multiple turns. This wastes tokens, costs money, and degrades user experience. The agent must detect loops early and redirect the LLM back to productive work.

## Decision

Loop detection is a **background valtron task** that monitors agent output for repetition. When a loop is detected, the agent loop is redirected using observation and reflection memory.

### Detection Strategy

```
Loop Detector Task
├── Monitors: Agent output stream (assistant messages)
├── Tracks: Last N message contents (sliding window)
├── Algorithm: Content similarity comparison
│   ├── Exact match → immediate detection
│   ├── Near-match (fuzzy hash similarity) → threshold-based detection
│   └── Structural match (same tool calls repeated) → tool call pattern detection
└── On detection: Signal agent loop to redirect
```

### Detection Methods

| Method | What It Detects | Algorithm | Threshold |
|--------|-----------------|-----------|-----------|
| **Exact match** | Identical text repeated | String equality | 2 consecutive identical messages |
| **Fuzzy similarity** | Near-identical text | SimHash / MinHash comparison | > 90% similarity over 3 messages |
| **Tool call repetition** | Same tool calls repeated | Tool call name + argument comparison | Same tool call pattern 3+ times |
| **Token budget exhaustion** | LLM spinning without progress | Token count per turn without tool results | > 80% context used, no tool results in last 3 turns |

### Detection State

```rust
pub struct LoopDetector {
    /// Sliding window of recent assistant messages
    window: VecDeque<ModelOutput>,  // foundation_ai::types::ModelOutput
    
    /// Window size (number of messages to track)
    window_size: usize,
    
    /// Similarity threshold for fuzzy matching (0.0 - 1.0)
    similarity_threshold: f32,
    
    /// Consecutive exact match count
    exact_match_count: usize,
    
    /// Tool call pattern tracker
    tool_call_history: VecDeque<Vec<ToolCallSignature>>,
}
```

pub struct ToolCallSignature {
    pub tool_name: String,
    pub argument_hash: u64,  // hash of arguments for comparison
}
```

### Detection Algorithm

```rust
impl LoopDetector {
    /// Check if the latest assistant message indicates a loop
    pub fn check(&mut self, message: &ModelOutput) -> LoopDetection {
        self.window.push_back(message.clone());
        while self.window.len() > self.window_size {
            self.window.pop_front();
        }

        // 1. Exact match check (last 2 messages)
        if self.window.len() >= 2 {
            let last = self.window.back().unwrap();
            let prev = self.window.iter().rev().nth(1).unwrap();
            if last == prev {
                self.exact_match_count += 1;
                if self.exact_match_count >= 2 {
                    return LoopDetection::ExactLoop {
                        repeated_message: last.clone(),
                        repetitions: self.exact_match_count,
                    };
                }
            } else {
                self.exact_match_count = 0;
            }
        }

        // 2. Fuzzy similarity check (last 3 messages)
        if self.window.len() >= 3 {
            let messages: Vec<_> = self.window.iter().rev().take(3).collect();
            let avg_similarity = self.average_pairwise_similarity(&messages);
            if avg_similarity > self.similarity_threshold {
                return LoopDetection::FuzzyLoop {
                    similarity: avg_similarity,
                    messages: messages.into_iter().cloned().collect(),
                };
            }
        }

        // 3. Tool call repetition check
        if self.tool_call_history.len() >= 3 {
            let patterns: Vec<_> = self.tool_call_history.iter().rev().take(3).collect();
            if self.all_patterns_equal(&patterns) {
                return LoopDetection::ToolCallLoop {
                    pattern: patterns[0].clone(),
                    repetitions: 3,
                };
            }
        }

        LoopDetection::NoLoop
    }
}
```

### Response to Loop Detection

When a loop is detected, the agent loop is redirected:

```
Loop detected
├── Inject redirect message into message list:
│   "I notice I've been repeating myself. Based on my observations and reflections,
│    I should instead: [summary from reflection memory]"
├── If loop persists after redirect (2nd detection):
│   ├── Try a different model (e.g., from claude-sonnet-4-6 to claude-opus-4-8)
│   └── OR adjust temperature (increase for more diversity)
├── If loop persists after model change (3rd detection):
│   └── Terminate session with loop detection error
```

### Redirect Message Construction

The redirect message is constructed from memory:

```rust
impl LoopDetector {
    /// Construct a redirect message from memory state
    pub fn build_redirect(
        &self,
        working_memory: &WorkingMemory,
        reflection_memory: &ReflectionMemory,
        observation_memory: &ObservationMemory,
    ) -> Messages {
        let context = format!(
            "Based on the session context:\n\
             Working memory: {}\n\
             Recent reflection: {}\n\
             Recent observations: {}\n\n\
             You appear to be in a repetition loop. Please change your approach.",
            working_memory.summary(),
            reflection_memory.latest_summary(),
            observation_memory.latest_summary(),
        );

        Messages::User {
            role: MessageRole::System,
            content: UserModelContent::Text(TextContent {
                content,
                signature: None,
            }),
            signature: None,
        }
    }
}
```

### Valtron Task Integration

The LoopDetector runs as a `sequenced` valtron task alongside the agent loop:

```
Agent Loop (execute)
└── Sequenced with:
    └── Loop Detector (sequenced)
        ├── Reads agent output from shared buffer
        ├── Checks for repetition
        └── If detected:
            ├── Returns TaskStatus::Ready(LoopDetection::Detected)
            └── Agent loop processes via map_circuit short-circuit
```

### Configuration

```rust
pub struct LoopDetectorConfig {
    /// Number of messages to track in sliding window
    pub window_size: usize,           // default: 5
    
    /// Fuzzy similarity threshold (0.0 - 1.0)
    pub similarity_threshold: f32,    // default: 0.9
    
    /// Maximum tool call repetitions before detection
    pub tool_call_max_repeats: usize, // default: 3
    
    /// Maximum consecutive redirects before termination
    pub max_redirects: usize,         // default: 3
    
    /// Whether to try model change on persistent loops
    pub try_model_change: bool,       // default: true
    
    /// Temperature adjustment on persistent loops
    pub temperature_delta: f32,       // default: +0.3
}
```

> **RESOLVED (2026-06-15, F28):** Loop detection is an **output processor** (runs inline after each turn), not a separate parallel task. Redirect-from-memory uses observations/reflections.

## Rationale

**Why a separate valtron task instead of inline detection?**  
- Detection should not block the agent loop — it runs in parallel
- Valtron's progress model allows detection to report "checking" vs "detected" states
- Detection can be toggled on/off without changing the agent loop

**Why multiple detection methods?**  
- LLMs can loop in different ways — exact repetition, near-repetition, and tool call repetition
- A single method misses some loop types
- Combining methods provides comprehensive coverage

**Why use memory for redirect messages?**  
- The redirect must reference the actual session context, not generic advice
- Working memory + reflection memory provide the distilled session state
- This gives the LLM the information it needs to change its approach

**Why try model change as a last resort?**  
- Different models have different generation patterns — a model change can break the loop
- Model changes are expensive (different API, different pricing)
- Only used after redirect fails — escalation strategy

## Alternatives Considered

### No loop detection
- **Pros:** Simpler, no false positives
- **Cons:** Wasted tokens, poor user experience when LLM loops
- **Rejected because:** Loops are a known LLM failure mode — must handle gracefully

### Client-side loop detection (UI detects and interrupts)
- **Pros:** No server-side complexity
- **Cons:** UI can't see tool call patterns, only text output
- **Rejected because:** Server-side detection has access to full message history and tool call data

### Embedding-based similarity for fuzzy detection
- **Pros:** Semantic similarity, not just text similarity
- **Cons:** Requires embedding generation for every message — expensive
- **Deferred to:** Optimization phase — start with SimHash, add embedding-based detection if needed
