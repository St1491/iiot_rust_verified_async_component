# Verification Report
This report documents the verification strategy behind the async fail-safe actor described in the [README](README.md). The component implements a four-state finite state machine ([`Idle`](README.md#idle) → [`Initialized`](README.md#initialized) → [`Acting`](README.md#acting) → [`Broken`](README.md#broken)) whose transitions are enforced at compile time by the Rust type system, and whose input-handling logic ([`Config::new`](README.md#config), [`Config::parse`](README.md#parse)) is verified with two complementary Rust-native tools: [Kani](README.md#kani) (bounded model checking) and [Creusot](README.md#creusot) (refinement-type proof).

Each section states a design choice, the reasoning behind it, and (where applicable) a concrete example of a property that the chosen mechanism catches or misses.
## 1. Verification strategy
The [`Actor::run`](README.md#run-1) method is async because it models a realistic industrial workload: the actor sleeps for a configured duration while performing work, and other actors must be able to proceed concurrently. `tokio::time::sleep` is used to simulate the work window.

Both Kani and Creusot currently lack support for verifying async functions with await points, and neither can inspect `<dyn State>` dispatch. To work within this limitation, I structured the code so that state transitions, resource reservation and error handling live in synchronous methods (`check_integrity`, `act`, `stop`, [`reserve_resource`](README.md#reserve_resource), [`free_resource`](README.md#free_resource)) that are verifiable in principle, even though the `<dyn State>` dispatch ultimately prevents the tools from reaching them; while the waiting (the actual work duration) is isolated in the async run method. The safety-critical logic is therefore separated from the concurrency concern, which is the part the tools cannot yet reach.
## 2. Tool selection
I chose Kani to verify [`parse`](README.md#parse) because it is a pure, input-driven, synchronous function with no loops over unbounded data. Kani's bounded model checker explores arbitrary symbolic inputs to the string arguments and confirms that every input either parses into a valid [`Config`](README.md#config) or panics deterministically. This is exactly the kind of input-exhaustiveness that bounded search over concrete values covers.


I chose Creusot to verify the [`new`](README.md#config) and [`parse`](README.md#parse) functions, as they are pure and synchronous, with a clear input→output relationship that maps directly onto a refinement-type contract. Where Kani confirms the property over a (bounded) set of generated inputs, Creusot proves the pre/post-conditions hold for all possible inputs unboundedly. Together the two give complementary assurance: Kani catches a concrete bad input, Creusot rules out an entire class of them by construction.


I did not attempt to verify the State trait or the `Actor::run` method with either Kani or Creusot. Both tools currently cannot introspect `<dyn State>` trait-object dispatch, and neither supports verifying async functions containing await points. The state-transition safety is therefore delegated entirely to the Rust type system, which is the correct boundary: the invariants are structural and are enforced at compile time, not at runtime.
## 3. Compile-time invariants
I enforced [invariants](README.md#typestate-invariants) through the return type `(Result<State, String>)` rather than via sealed traits or phantom type parameters. The `Result` type already makes invalid transitions unrepresentable: a call-site cannot pattern-match on an [`Acting`](README.md#acting) state returned from `Idle::act()` because the `Ok` variant is not produced. A sealed trait on [`State`](README.md#state) would additionally prevent external crates from implementing a new state, but since the trait is already crate-private, this is redundant. I chose the simpler mechanism because the visibility boundary already closes the hole a sealed trait would guard against.\
The `check` method on [`Idle`](README.md#idle) currently returns `true` unconditionally. This is a deliberate placeholder: the typestate pattern guarantees that check must be called before the actor can act, but the content of the check itself is domain-specific and outside the scope of this project. In a production system this would query a hardware health register or a cryptographic attestation.
## 4. Limitations
The [`reserve_resource`](README.md#reserve_resource) / [`free_resource`](README.md#free_resource) / [`run`](README.md#run) / [`Actor::run`](README.md#run-1) methods are async and involve a Mutex shared across concurrent actors. Neither Kani nor Creusot can verify these paths. The correctness of the resource protocol (reserve → act → free) is therefore a convention enforced by the code structure and documented via the CAUTION note, not a guarantee enforced by a verifier. This is the primary gap in the current verification coverage.\
A concrete example: if [`free_resource`](README.md#free_resource) were called twice for the same reservation (a logic error in [`Actor::run`](README.md#run-1)), the Mutex-protected counter would silently exceed `shared_resources`. Neither [Kani](README.md#kani) (bounded, synchronous-only) nor [Creusot](README.md#creusot) (cannot model async Mutex contention) would detect this. The final `assert_eq!` in [`run`](README.md#run) [[lib.rs]](src/lib.rs) is the only guard, and it fires only after all actors have completed.
## 5. Multi-actor protocol and hyperproperties
The multi-actor simulation ([`run`](README.md#run)) exercises this boundary: N actors asynchronously compete for M shared resources through a Mutex, and the resource protocol (reserve → act → free) is the interaction pattern whose correctness depends on the synchronous invariants above plus the runtime Mutex discipline. A natural hyperproperty this enforces across the full execution trace is mutual exclusion: at no point in the trace is a resource held by more than one actor. This is currently guaranteed by the Mutex and the reserve/free pairing, not by a formal verifier, which is the primary direction for future work.
## 6. Conclusions
The correctness of the FSM is guaranteed by the Rust type system alone: neither verification tool can verify this implementation because they do not support async functions and cannot directly introspect the `<dyn State>` field of the Actor.\
What the tools do certify is that the user-facing entry points ([`new`](README.md#config), [`parse`](README.md#parse)) behave as specified for all inputs: valid input produces the expected output, and invalid input panics in a controlled manner without leaving the system in a harmful state.\
Closing the remaining gaps requires either a verifier with async/dyn-dispatch support or a refactoring that eliminates the `<dyn State>` indirection in favour of an enum-based state machine, which would be verifiable by both tools at the cost of losing the open-closed design of the current trait-based approach and its Rust-native approach.
## 7. AI Disclosure

**Tool:** Qwen (Alibaba)\ 
**Model:** Qwen 3.8:27b\
**Accessed via:** Ollama + Open WebUI 

### Scope of AI assistance

The AI tool was used exclusively as a **review and documentation collaborator** during the later stages of the project. Specifically:

- Reviewing the project against the assignment deliverables and identifying gaps in the comparative report
- Suggesting structure for the README / REPORT split
- Drafting some sections for both README and REPORT
- Reviewing the Rust source code for potential missed bugs
- Reviewing the README for clarity, consistency, and completeness

### What was done independently

The following were completed without AI assistance:

- All design decisions: the FSM structure, the typestate pattern, the resource protocol, the choice of trait-based dispatch over enum-based state, the two-mode architecture
- All Rust implementation code
- The decision to assign `parse` to Kani and `new` + `parse` to Creusot, and to leave the FSM to the type system
- Writing and running all Kani proofs
- Writing and running all Creusot contracts
- Interpreting all verification results and confirming correctness
- Writing the native Rust tests
- The initial README draft and all code documentation

### Accountability

I can explain every design decision, every line of code, every verification contract, and every sentence in this report from my own understanding. The AI was used to structure and refine the written analysis after the technical work was complete. I take full responsibility for the correctness, originality, and academic integrity of this work.