# Future Consideration: DMs as Private Threads

**Current State**: DMs are implemented as simple linear encrypted chat messages.

**Alternative Architecture**: Convert DMs to private threads
- Each DM conversation becomes a private thread (only visible to two participants)
- Messages become posts that can have replies
- Can use same graph/chronological/list UI as public threads
- Posts in DM threads are encrypted
- Allows threaded conversations in DMs
- More consistent with rest of application architecture

**Decision needed**: Should we refactor DMs to be private threads, or keep them as simple chat?

**Pros of private threads**:
- Consistent UI/UX with rest of app
- Threaded conversations in DMs
- Reuse existing thread infrastructure
- Better for complex discussions
- Graph view for DM conversations

**Cons**:
- More complex to implement
- Might be overkill for simple messaging
- Current implementation works for linear chat
- Need to encrypt entire thread/post structure

**Note**: Currently tabled for future consideration. Focus shifted to LLM agent integration.
