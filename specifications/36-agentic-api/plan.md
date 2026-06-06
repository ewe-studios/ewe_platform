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
│ │
│ ├─ INNER LOOP (tool calls + steering)
│ │ ├─ Drain steering messages → inject into context
│ │ ├─ streamAssistantResponse()
│ │ │ ├─ transformContext(messages)
│ │ │ ├─ convertToLlm(messages)
│ │ │ ├─ streamSimple(model, context)
│ │ │ └─ Emit: message_start → message_update* → message_end
│ │ ├─ If error/aborted → emit(turn_end, agent_end), return
│ │ ├─ Extract tool calls
│ │ ├─ If tool calls:
│ │ │ ├─ executeToolCalls() → sequential or parallel
│ │ │ │ ├─ prepareToolCall() → validate, beforeToolCall
│ │ │ │ ├─ execute() → run tool with abort signal
│ │ │ │ └─ finalizeExecutedToolCall() → afterToolCall
│ │ │ └─ Emit: tool_execution_start → tool_execution_update* → tool_execution_end
│ │ │ → message_start/end for each tool result
│ │ └─ Check steering queue → repeat inner loop if messages
│ │
│ └─ Drain follow-up queue → set as pending, continue outer loop
│
└─ emit(agent_end, newMessages)

## Mastra Own Approach

┌─────────────────────────────────────────────────────────────────┐
│ Agent.generate() / .stream() │
└─────────────────────────────────────────────────────────────────┘
│
▼
┌─────────────────────────────────────────────────────────────────┐
│ 1. Resolve dynamic config (instructions, model, tools, etc.) │
└─────────────────────────────────────────────────────────────────┘
│
▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. Build Input Processor Workflow │
│ - Merge configured + memory auto-processors │
│ - Deduplicate by processor ID │
└─────────────────────────────────────────────────────────────────┘
│
▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. Execute Input Processors │
│ - WorkingMemory: Inject system message with WM data │
│ - MessageHistory: Load last N messages │
│ - SemanticRecall: Query vector store for similar messages │
│ - ObservationalMemory: Activate observations if threshold │
└─────────────────────────────────────────────────────────────────┘
│
▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. Call LLM (MastraLLM.doGenerate() / doStream()) │
│ - Model routing if multiple models configured │
│ - Tool execution loop if tools are called │
│ - Processor intervention handling │
└─────────────────────────────────────────────────────────────────┘
│
▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. Execute Output Processor Workflow │
│ - MessageHistory: Save new messages │
│ - SemanticRecall: Create embeddings for new messages │
│ - ObservationalMemory: Check observation/reflection trigger │
└─────────────────────────────────────────────────────────────────┘
│
▼
┌─────────────────────────────────────────────────────────────────┐
│ 6. Return result (GenerateTextResult / StreamTextResult) │
└─────────────────────────────────────────────────────────────────┘

## What I am going for?

I have some overall set of ideas of how I want things to work, so generally here are them summarized:

1. All AI interactions are always serializable to JSON and Arrow format (if not we can do flatbuffers, but arrow preferable)

2. These messages like the current foundation_ai are always communicated via the `Next()` type of `Stream` but we should also not ignore the fact `Stream::Pending(T)` provides us unique values to communicate states that might not be directly recorded where the interactions are but can be used to feed UI, or other processes with rich information about what's current going on.

3. We will build the agentic loop on valtron task infrastructure, just as we already do for foundation_ai AI/LLM interactions and computation, it just so suited for this so well with it's reach iterator/generator style execution model.

4. Reasoning alot about this, i think we need a few components especially after reading how Pi, Hermes and Mastra works.

In my mind, every start of a Agent requires the provision of a SessionId which will forever be its representation, the nice part of this, it returns a struct that has a few things:

1. Messages - The message API where all interactions go to for serialization and persistence, messages will have underlying queue hooks (probably say a Broadcaster from foundation_core synca module) so it can pubsub and alert multiple listeners (e.g other valtron tasks) to process incoming messages. It also has a Vector search capability and also integrates /home/darkvoid/Boxxed/@formulas/src.rust/src.FileSystemAPIs/src.Search/fff as well for fast grep/ripgrep to search files on a local filesystem.
2. Context - The core context API which will own retrieval of memory, context - it should probably own all the different types of memory we wish to support (see below) and also underlying own the vector search capabilities we want to support checking these memories (WorkingMemory, ObservationMemory, ReflectionMemory). Also integrates fff for ripgrep like search over files and file content. See fff source here: /home/darkvoid/Boxxed/@formulas/src.rust/src.FileSystemAPIs/src.Search/fff
3. ToolCallManager - A core API which will own tool call execution and will be given tool call messages (interactions) and will own and handle the execution, returning results to the agent task so it can yield those first to the Messages API instance, then give it to the AI once successfully saved and persisted. this ensures all interactions was successfully persisted for the session before any future processing occurs. The important thing to note is this has internal tool call management capabilities so tool calls can be queued, executed in the background and will only stop if interrupted else will deliver the results once all tool call as finished being processed, it handles parrallel, sequential execution of tool calls, and knows how to split tool calls into groups where if some tool calls must be sequentially executed before others can be executed in parralel and knows how to manage this.
4. PriorityQueue - A (concurrent-queue backed delivery queue) core api which will own steering where if users wish to stop, interrupt, steer the LLM, then the agent will check if there are messages, will combine that and add these to the front so the AI must first answer this and we also when such a queue has messages, tell the ToolCallManager to cancel any ongoing tool calls not yet completed since users want to interrupt.
4. FollowUpQueue - A (concurrent-queue backed delivery queue) core api which will own follow up messages where if users wish provide future steering messages to the LLM after all tool calls, and current loop is finished, further giving the llm instructions for the next steps. Unlike the PriorityQueue, any messages in here, never interrupts the ToolCallManager or llm but wait till the next call, so it keeps going.
5. EmbeddingProvider - A APi which owns all embedding generation and probably fronts a concurrentQueue which it wraps to deliver embedding generation requests to some valtron task which will just owns this and owns the cache system that its configured to use (e.g fjall, memory, file etc) or just an in-memory LRU cache that has a max size and will never allow unbounded embeddings be kept in memory for long.

In my mind each of these is backed by a valtron task which knows how to manage the different concerns they have, since valtron allows both sequential, linked execution, broadcasted tasks for parrallel execution, priority (execute to finish then continue with me semantics, execute me and this child turn by turn), we can architecture these nicely, cleanly and logically, breaking different complex parts into different valtron tasks that each use to achieve what it needs.

But something is important in my mind and we should also add this into our valtron skill, where 

Yes, we valtron it all.

### Message API

Messages: A store which is tied to a specific session via a unique session id, the benefit for this is, restarting a session is as simple as providing the session id and you get repeatable and replayable and continuation all in one. We can even use scru128 ids instead to make them time ordered which lets us list sessions in a session store automatically by just normal sorting and see which session started first. We can provide a nice API method that uses can use e.g a function takes a custom user friendly name and uses it to compute a time ordered id similarly to scru128 that is sortable, unique and if it includes the machine details becomes globally unique.

In my mind we do the same thing we've being done, we define Messages as a trait that defines the contract for what the messages API provides e.g

a. Get the last 5 messages (no need for AI to grep stuff)
b. Get all messages from the start of time, each message also has a scru128 id which makes them naturally ordered by just the id.
b. It stores every interaction from the AI with the user e.g tool calls, tool call request, AI assistant replies, user messages, user interupt messges, steering messages, thinking blocks.

This allows the Message API to be the defacto shareable (probably put it a Arc or better yet have a MessageInner that is wrapped in a Arc with a nicer Message struct wrapper that) with our scaffold macro this has become super easy to do.

The messages will contain every message since the lifetime of that interaction and we will never compact it, its an audit of every single interaction for that session.

More so, we should be able to do semantic recall (bascially a vector search) on the session to find important information that is really important to the current interactions.

Different operations can then get the copy of the Message{inner: Arc<MessageInner>} and use it to perform different operations on the messages, like:

### Context Memory APIs 

The Context API provider should own multiple Memory APIs:

1. A WorkingMemory - which is a simple API that the AI generates which we either device when user interacts with the AI or using a smaller model to look at the session messages to generate important facts about the user, this is small, and always just kept to the latest important fact about the user what they like, how they want to work, who they are, facts about them and we keep cleaning it up, removing any outdated information.

2. Observable Memory - which builds up the memory and represent time scoped details about the Session - what we are working on, where we left off, what the users asked last, what the user asked us to remember (that gets saved to working) memory, never really tool calls actual output but if important a summarization of what some important result from a tool call has that is really important to the user's session.

The observable memory API will have internal Semantic recall via vector search capabilities that help find relevant information related recorded in observable memory by the llm.

Goal: to create structured observations from recent conversations, where it has clarity on whats an assertions and whats a question, when it was said (timestamp) and where its referenced, ensuring to preserve unusual phrasing, ensures to use precise verbs and maintains important details that are relevant.

3. Reflection Memory - the reflection memory is the summarization of the Observable Memory, this allows us to better reduce large verbose memory down to more smaller, summarization of what was observed. The reflections will also be stored in the session Messages with a {message_type: observation, ...} which indicates its an observation and not just a AI interactions and will generally be skipped by agents when fetching interaction content but because super useful to remember where we were, what was the last condensation of the overall session when last we observed and reflected. Reflections also are saved to the Message store (could be a different table, file, whatever or even in the same file generally for files i reason a NDJSON, JSONL file where each line is a JSON object). But reflections will generally be generated and pushed to the Messages persistent layer after they are generated so they are beside the Observation or atleast close to it, and they are the summarization of long observation which can also contain references to the messages scru128 ID which lets us quickly fast track to where its located (how we store it does not matter, we will use the full capabilities we already have in foundation_db with regards where its stored).

Once observations are reflected on, they replace the content of Observable Memory reducing the context once again and letting AI pull just relevant contextual details + what comes from WorkingMemory (which are always short, small, important facts about user, session, short intro of things to always keep in mind).

In my mind, we always when we update working memory also push into the session storage with a message-type: working memory to allow us refresh that when we get the session or hydrate again.

Goal: The reflections becomes the entire observation memory once generated replacing everything in there, it recorganises the observation memory, ensuring to preseve dates/times when present, combine items, condesne older observations more aggressively, retain more details for recent observations.

The can optimize how these is pulled.

Mastra triggers:

- Generation of observation memory when session recent interactions goes past 30k
- Generation of reflections when observation memory goes past 40k,

### Embeddings API

Note: The context API should also own the get a shared Embedding API which will own embedding generation and caching (see aobve for context).

When we generate embeddings, we should cache with a data strcuture to perform a LRU cache say with 1000 items with most recent at the top and least used at the bottom allowing us re-use embeddings for texts. We might need to investigate whats the optimal way for this to ensure we are reusing as much token generated either by word or by sentence or something.

We need to ensure our embeddings are generated with the exact same dimensions as different embedding models use different dimensions which will cause issues if we mix them.

Ensure cache has max size and eject anything older.

Maybe fjall can be helpful here with its LSM so we can create optimal keyspaces.

### Loop Detection

If we see the AI repeating the same content over and over then its probably in a loop and we need to stop it and ensure to give it a message to redirect it back to what it should be doing based on our observations and reflections.

We detect it, remove the duplicates, retry and if it happens again maybe use a different model or temperature.

## Vector Search

We will need to implement sharead modules that implement vector search for both in-memory stores, turso (which has native vector schema types) and see if we can build such on fjall or if there are rust based vector crates that work both in native, wasm and web to make it possible to run this across these environments. e.g pinecone, chroma.

This then allows us build semantic recall capabilities on this, we should definitely add these vector stores in foundation_db so others can use them.

We should learn as much as we can, i have source code for vector databases here: /home/darkvoid/Boxxed/@formulas/src.rust/src.FileSystemAPIs/src.VectorDB/ (especially: /home/darkvoid/Boxxed/@formulas/src.rust/src.FileSystemAPIs/src.VectorDB/src.Chroma) - There is alot we can learn from it.
