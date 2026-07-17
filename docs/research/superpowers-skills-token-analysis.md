# Are Superpowers Skills Token Gluttons?

> A thorough analysis of how skill definitions, tool schemas, and system prompt overhead consume tokens in Claude — and whether the cost is worth it.

---

## The Short Answer

Your ~45 skills cost roughly **5,700 tokens** out of a total system prompt overhead of **~80,000 tokens**. That's about **7% of the system prompt** and **2.8% of a 200K context window** — or a modest **0.6% of a 1M window**. The skills are not the gluttons. The *infrastructure around them* is.

## What Actually Eats Tokens

Every Claude.ai turn carries a massive invisible payload before your message is even processed. Here's the breakdown for a setup like yours:

| Component | ~Tokens | % of Overhead |
|---|---:|---:|
| Tool definitions (all function JSON schemas) | 18,000 | 22.4% |
| Memory filesystem rules & examples | 12,000 | 14.9% |
| Core behavior instructions | 8,000 | 10.0% |
| Search + copyright compliance | 6,000 | 7.5% |
| **Skills listing** | **5,700** | **7.1%** |
| API-in-artifacts instructions | 3,500 | 4.4% |
| Computer use + file handling | 3,000 | 3.7% |
| Safety, formatting, wellbeing, etc. | ~24,000 | 30.0% |
| **Total fixed overhead** | **~80,000** | **100%** |

The real gluttons are the **tool schemas** (~18K tokens for all the JSON function definitions — `web_search`, `bash_tool`, `memory_*`, `places_search`, `visualize`, etc.) and the **memory filesystem rules** (~12K tokens of detailed instructions for how to read, write, and manage persistent memory).

Your skills listing — the `<available_skills>` block with 45 entries showing name, description, and path — is a mid-tier consumer at ~5,700 tokens.

## The Compounding Problem

These 80K tokens aren't a one-time cost. The LLM API is **stateless**: every single turn resends the entire context. Turn 1 sends 80K of overhead plus your message. Turn 15 sends 80K of overhead plus the entire conversation history plus your message. By turn 30, the total input can approach 150K–200K tokens.

Research findings from multiple independent analyses confirm this pattern:

- One tracked session showed a user's 14-token question costing $0.0018 at turn 1, but **$2.41 at turn 260** — a 1,339× increase from accumulated context alone.
- In a typical session, the user's own messages constitute only about **1.3% of all tokens processed**. The other 98.7% is system prompt, tool definitions, and replayed history.
- A "hi" prompt in Claude Code consumed roughly **31,000 tokens** — all infrastructure overhead.

## The Caching Safety Net

Prompt caching is the mechanism that makes large system prompts financially viable. Anthropic caches the stable prefix of each request:

- **First request**: full input price (plus a 25% write premium on cached portions)
- **Every subsequent request within the TTL**: cached portions cost **10% of the base input price** — a 90% discount

Since the system prompt, tool definitions, and skills listing are *identical across turns in a conversation*, they're prime caching candidates. In practice, after turn 1, those 80K tokens of overhead cost roughly what 8K tokens would normally cost. The economics flip dramatically: a 50,000-token system prompt across 500 requests drops from $75/day to ~$7.69/day with caching.

However, caching doesn't save *context window space*. Those 80K tokens still occupy the window regardless of whether they were read from cache or processed fresh. In a 200K window, 40% is spoken for before you type.

## Skills vs. MCP Tools: The Deferred Loading Advantage

Your setup actually uses **two different mechanisms** for extensibility, and they have very different token profiles:

**Skills (your ~45 entries):** Only the *catalog listing* loads — name, description, and file path per skill. The actual `SKILL.md` content (which can be thousands of tokens each) only loads when you `view` the file during a task. This is essentially manual lazy-loading. Cost: ~5,700 tokens total for the entire catalog.

**MCP/Deferred tools (Alma Spirit, Claude-in-Chrome):** These use `tool_search` with `defer_loading`, meaning only the **tool search function itself** (~500 tokens) loads initially, plus a brief listing of tool names and descriptions. Full tool schemas load on demand. Without deferral, Alma Spirit's 17 tools and Claude-in-Chrome's 22 tools would add another ~10,000–15,000 tokens of JSON schemas. With deferral, they add perhaps ~1,000 tokens.

Anthropic's own benchmarks show **85% reduction in token usage** from tool search deferral, and accuracy on MCP evaluations *improved* (Opus went from 49% to 74%) because fewer tool definitions in context means less confusion during tool selection.

## Is It Worth It?

The cost-benefit breaks down along three axes:

### Context Window Pressure
On a 200K window (Claude Code, older models), 80K of overhead leaves only 120K for actual work. That's meaningful pressure. On a **1M window** (Sonnet 5, Sonnet 4.6 in claude.ai), 80K is 8% — plenty of room. Your skills specifically consume 0.6% of 1M. Negligible.

### Financial Cost
With prompt caching, the per-turn cost of all 80K tokens of overhead after turn 1 is roughly equivalent to processing 8K tokens. Your skills' share of that: ~570-token-equivalent cost. At Sonnet 4.6 pricing (~$3/MTok input), that's about **$0.0017 per turn** for your entire skills catalog. Truly negligible.

### Capability Value
Each skill acts as a routing hint — a few hundred tokens that tell the model *which expert file to read* for a given task. Without the skills listing, the model would either need to search blindly, ask you which workflow to follow, or carry every skill's full content in context simultaneously (which would cost 50–100x more). The catalog approach is the efficient architecture.

## The Real Token Sinks to Watch

If you're hunting for gluttony, these are the bigger targets:

1. **Long conversations**: Every turn replays the full history. Clear or start fresh when switching topics.
2. **Tool call outputs**: A single `bash_tool` returning 10,000 lines of output stays in context *for every subsequent turn*. One verbose command can cost more than your entire skills catalog.
3. **File reads via `view`**: Reading a 500-line file adds those tokens to context. Read surgically.
4. **Extended thinking**: Thinking tokens are billed as output tokens (5× input pricing on some models). A default thinking budget of tens of thousands of tokens per request can dwarf everything else.
5. **MCP tool schemas (without deferral)**: Without `tool_search`, connecting 5+ MCP servers can consume 50,000–70,000 tokens. Your setup wisely uses deferral.

## Conclusion

Your ~45 superpowers skills are **not** token gluttons. At ~5,700 tokens, they represent a lean catalog index — roughly the size of one medium web page. The architecture is sound: small descriptions up front, full content loaded on demand. The real overhead comes from the platform infrastructure itself (tool schemas, memory rules, safety instructions) which you don't control, and from conversation accumulation patterns which you do.

The skills are a routing table, not a buffet. They cost pennies per conversation and earn their keep by preventing far more expensive mistakes — like loading the wrong workflow or asking unnecessary clarifying questions.

*Estimated figures based on character-count heuristics (~4 chars/token) and published research. Actual tokenization varies by model and content.*
