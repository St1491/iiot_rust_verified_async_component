# Verification Report
This report documents the verification strategy behind the async fail-safe actor described in the [README](README.md).\
The component implements a four-state finite state machine ([`Idle`](README.md#idle) → [`Initialized`](README.md#initialized) → [`Acting`](README.md#acting) → [`Broken`](README.md#broken)) whose transitions are enforced at compile time by the Rust type system, and whose pure functions ([`Config::new`](README.md#new), [`Config::parse`](README.md#parse), and the [`ResourcePool`](README.md#resourcepool) protocol) are verified with two complementary Rust-native tools: [Kani](README.md#kani) for bounded input-exhaustiveness, [Creusot](README.md#creusot) for unbounded correctness, each assigned the properties it is best suited to verify.

Each section states a design choice, the reasoning behind it, and (where applicable) a concrete example of a property that the chosen mechanism catches or misses.
## 1. Verification strategy
The [`run_one`](README.md#run_one) method is async because it models a realistic industrial workload: the actor sleeps for a configured duration while performing work, and other actors must be able to proceed concurrently. `trpl::sleep` is used to simulate the work window.

Creusot currently lacks support for verifying async functions with await points. To work within this limitation, I structured the code so that state transitions, resource reservation and error handling live in synchronous methods (`check_integrity`, `act`, `stop`, [`reserve_resource`](README.md#reserve_resource), [`free_resource`](README.md#free_resource)) that are verifiable in principle; while the waiting (the actual work duration) is isolated in the async [`run_one`](README.md#run_one) method. The safety-critical logic is therefore separated from the concurrency concern, which is the part the tools cannot yet reach.
## 2. Tool selection
I chose [Kani](README.md#kani) to verify [`parse`](README.md#parse) because it is a pure, input-driven, synchronous function with no loops over unbounded data. Kani's bounded model checker explores arbitrary symbolic inputs to the string arguments and confirms that every input, within its bound that covers most realistic input values, either parses into a valid [`Config`](README.md#config) or panics deterministically. This is exactly the kind of input-exhaustiveness that bounded search over concrete values covers.
I also used Kani to verify the resource-management functions ([`reserve_resource`](README.md#reserve_resource) and [`free_resource`](README.md#free_resource)) since they are synchronous and have a concrete arithmetic invariant (the counter never exceeds capacity) that bounded checking covers directly. 

I chose [Creusot](README.md#creusot) to verify the [`new`](README.md#new) function, as it is pure and synchronous, with a clear input→output relationship that maps directly onto a refinement-type contract. I did not assign [`parse`](README.md#parse) to Creusot because it calls `std::str::parse`, which Creusot cannot model; `parse` is, therefore, verified by Kani instead (see below), where `std` functions are within its supported surface. I also assigned Creusot contracts to the [`ResourcePool`](README.md#resourcepool) methods ([`reserve`](README.md#reserve), [`free`](README.md#free)): each has a precondition (the counter is in range) and a postcondition preserving the invariant `current_pool` <= `max_capacity`. This gives an unbounded proof that the resource arithmetic cannot overflow the configured bound, complementing Kani's bounded exploration of the same logic. Where Kani confirms the property over a (bounded) set of generated inputs, Creusot proves the pre/post-conditions hold for all possible inputs unboundedly. Together the two give complementary assurance: Kani catches a concrete bad input, Creusot rules out an entire class of them by construction.


I did not attempt to verify the [`Actor`](README.md#actor) state transitions with either Kani or Creusot. The state transitions are enforced at compile time by the type parameter (invalid transitions cannot be written); the [Rust-native tests](README.md#rust-native-tests) confirm that each valid transition produces the expected state. As for [`run_one`](README.md#run_one), its critical synchronous logic ([`reserve_resource`](README.md#reserve_resource), [`free_resource`](README.md#free_resource)) is verified by Kani. The async orchestration itself (the `trpl::sleep` await, the retry loop) is beyond Kani's reach.
## 3. Compile-time invariants
I enforced [invariants](README.md#typestate-invariants) through the type parameter `S` on [`Actor<S>`](README.md#actor): each state is a distinct zero-sized type ([`Idle`](README.md#idle), [`Initialized`](README.md#initialized), [`Acting`](README.md#acting), [`Broken`](README.md#broken)), and the methods for a transition are defined only in the `impl` block for the source state. This means invalid transitions are not a runtime error, but a **compile error**.\
The `debug_check` function I call for the `check_integrity` method of the [`Idle`](README.md#idle) state currently returns `true` unconditionally. This is a deliberate placeholder: the typestate pattern guarantees that check must be called before the actor can act, but the content of the check itself is domain-specific and outside the scope of this project. In a production system this would query a hardware health register or a cryptographic attestation.
## 4. Limitations
The synchronous resource functions, [`reserve_resource`](README.md#reserve_resource), [`free_resource`](README.md#free_resource) for Kani and the [`ResourcePool`](README.md#resourcepool)'s [`reserve`](README.md#reserve) and [`free`](README.md#free) methods for both tools, are within reach, and I added tests for them: they verify that after a valid reserve→free cycle, the counter never exceeds the configured capacity. This is a bounded arithmetic property that Kani handles directly on one side, and an unbounded property that Creusot can prove on the other, demonstrating that the tools' reach extends beyond [`Config`](README.md#config) parsing or initialization into the actor's resource-management logic.\
The async execution path ([`run_one`](README.md#run_one) and [`run`](README.md#run)), in contrast, is beyond the scope of both tools: Creusot does not support async functions yet; as for Kani: although its documentation indicates experimental support for an async executor, I attempted to point it at [`run_one`](README.md#run_one) and [`run`](README.md#run) (both contain `await` points and reference `trpl::sleep`). The proofs did not complete and were terminated by the OS.\
The following table summarises the verification coverage across all three mechanisms:
| Property | Kani | Creusot | Type system |
|---|---|---|---|
| [`new`](README.md#new): fields match inputs | -- (indirectly though `parse`) | Unbounded ✓ | Native tests ✓ |
| [`parse`](README.md#parse): valid-or-panic | Bounded ✓ | ✗ (`std::parse` unmodelled) | Native tests ✓ |
| [`parse`](README.md#parse): concrete edge cases | Bounded ✓ | ✗ (`std::parse` unmodelled) | Native tests ✓ |
| Pool invariant: `0 ≤ current ≤ max` | Bounded ✓ (arbitrary ops) | Unbounded ✓ | -- |
| No double-free | Bounded ✓ (`should_panic`) | Unbounded ✓ | `panic!` guard |
| State-transition safety | ✗ (vacuous) | ✗ (vacuous) | **Compile-time ✓** |
| Async execution (`run_one`, `run`) | ✗ (terminated) | ✗ (unsupported) | -- |
| Mutual exclusion (multi-actor) | ✗ | ✗ | Mutex + convention |

## 5. Multi-actor protocol and hyperproperties
The multi-actor simulation ([`run`](README.md#run)) exercises this boundary: N actors asynchronously compete for M shared resources through a Mutex, and the resource protocol (reserve → act → free) is the interaction pattern whose correctness depends on the synchronous invariants above plus the runtime Mutex discipline. A natural hyperproperty this enforces across the full execution trace is mutual exclusion: at no point in the trace is a resource held by more than one actor. This is currently guaranteed by the Mutex and the reserve/free pairing, not by a formal verifier. A formal verification could be the primary direction for future work.
## 6. Conclusions
The correctness of the FSM is guaranteed by the Rust type system alone: the state transitions are enforced at compile time by the `Actor<S>` typestate, which is outside the reach of any runtime verifier by design. What the tools do certify is that the user-facing entry points ([`new`](README.md#new), [`parse`](README.md#parse)) and the resource protocol ([`reserve_resource`](README.md#reserve_resource), [`free_resource`](README.md#free_resource)) behave as specified, within the bounded scope of Kani's exploration and the unbounded scope of Creusot's proof.\
Closing the remaining gaps, specifically the async execution path ([`run_one`](README.md#run_one)), requires a verifier with async/`await` support, which neither Kani nor Creusot currently provides for this use case.
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

- All design decisions: the FSM structure, the typestate pattern, the resource protocol, the two-mode architecture
- All Rust implementation code
- The decision to assign most of the tests to each respective tool, and to leave the FSM to the type system
- Writing and running all Kani proofs
- Writing and running all Creusot contracts
- Interpreting all verification results and confirming correctness
- Writing the native Rust tests
- The initial README draft and all code documentation

### Accountability

I can explain every design decision, every line of code, every verification contract, and every sentence in this report from my own understanding. The AI was used to structure and refine the written analysis after the technical work was complete. I take full responsibility for the correctness, originality, and academic integrity of this work.