This plan is an overview architecture for what the agentic API for foundation-ai looks like, after studying how Pi, Hermes, Mastra and Yoagent works:

See explorations here:

- /home/darkvoid/Boxxed/@dev/repo-expolorations/src.datastar/yoke
- /home/darkvoid/Boxxed/@dev/repo-expolorations/hermes
- /home/darkvoid/Boxxed/@dev/repo-expolorations/mastra
- /home/darkvoid/Boxxed/@dev/repo-expolorations/pi

Which create specific agentic inner and outer loops which is responsible for:

1. Executing requests against the LLM
2. Pulling previous related conversation from both memory and contextual sources
3. Identifying the tool calls required to be made
4. Execute the tool calls serialy, in parrallel or batch
5. Applying their results back into the llm message list and re-run the llm processing loop
6. Repeating this until no more tool calls are required and the llm is not making anymore output.
7. Some like mastra store all these interactions and generate multi-level memories: observation, reflections and long term summarized contents that can be recalled from immediate to long term memory in conjuction with llm compaction (summarization and removal of older tool calls that no more have contextual value or relevance).

Taking all this in, I have ideated a set of ideas for how agentic loop and execution should work with foundation-ai and valtron.


## Condensed Pi agentic loop

See a rendered diagram of the Pi's agentic loop belo

agent.prompt("Fix the bug")
  └─ runWithLifecycle()
       └─ runAgentLoop(prompts, context, config, emit, streamFn)
            └─ runLoop()
                 │
                 ├─ emit(agent_start)
                 ├─ emit(turn_start)
                 ├─ emit(message_start/end) for each prompt
                 │
                 ├─ OUTER LOOP (follow-up continuation)
                 │    │
                 │    ├─ INNER LOOP (tool calls + steering)
                 │    │    ├─ Drain steering messages → inject into context
                 │    │    ├─ streamAssistantResponse()
                 │    │    │    ├─ transformContext(messages)
                 │    │    │    ├─ convertToLlm(messages)
                 │    │    │    ├─ streamSimple(model, context)
                 │    │    │    └─ Emit: message_start → message_update* → message_end
                 │    │    ├─ If error/aborted → emit(turn_end, agent_end), return
                 │    │    ├─ Extract tool calls
                 │    │    ├─ If tool calls:
                 │    │    │    ├─ executeToolCalls() → sequential or parallel
                 │    │    │    │    ├─ prepareToolCall() → validate, beforeToolCall
                 │    │    │    │    ├─ execute() → run tool with abort signal
                 │    │    │    │    └─ finalizeExecutedToolCall() → afterToolCall
                 │    │    │    └─ Emit: tool_execution_start → tool_execution_update* → tool_execution_end
                 │    │    │         → message_start/end for each tool result
                 │    │    └─ Check steering queue → repeat inner loop if messages
                 │    │
                 │    └─ Drain follow-up queue → set as pending, continue outer loop
                 │
                 └─ emit(agent_end, newMessages)


## What I am going for?

I have some overall set of ideas of how I want things to work, so generally here are them summarized:

1. All AI interactions are  always serializable to JSON and Arrow format (if not we can do flatbuffers, but arrow preferable)
2. These messages like the current foundation_ai are always communicated via the `Next()` type of `Stream` but we should also not ignore the fact `Stream::Pending(T)` provides us unique values to communicate states that might not be directly recorded where the interactions are but can be used to feed UI, or other processes with rich information about what's current going on.
3. We built the agentic loop on valtron task infrastructure, just as we already do for foundation_ai AI/LLM interactions and computation, it just so suited for this so well with it's reach iterator/generator style execution model.
