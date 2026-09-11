from pprint import pprint

from brainfish import (
    BrainfishAgent,
    DeepInteractionGateway,
    GraphRAGEnvironmentBoard,
    MacroReportAgent,
    PredictiveHealingEngine,
    PruningAxiomEngine,
    SemanticReasoningBank,
)


def main() -> None:
    memory_bank = SemanticReasoningBank()
    board = GraphRAGEnvironmentBoard()

    memory_bank.remember("Restart the exhausted worker pool before the backlog saturates downstream services.")
    memory_bank.remember("Throttle retry storms by opening a circuit breaker on the failing dependency.")
    stale_axiom = memory_bank.remember(
        "Always retry immediately regardless of queue pressure.",
        metadata={"harmfulness": 0.95},
        strength=0.2,
    )

    board.add_entity("api-gateway", kind="service", severity=0.3)
    board.add_entity("worker-pool", kind="service", severity=0.8)
    board.add_entity("payment-db", kind="service", severity=0.9)
    board.link("api-gateway", "worker-pool", relation="sends_jobs_to", weight=0.8)
    board.link("worker-pool", "payment-db", relation="writes_to", weight=0.9)

    operator = BrainfishAgent("ops-healer", "devops-self-healing", memory_bank, board)
    operator.observe("incident-42", "Worker pool reports thread starvation and rising retry pressure.", severity=0.85)
    board.link("incident-42", "worker-pool", relation="impacts", weight=0.95)

    healing = PredictiveHealingEngine(board, memory_bank)
    pruning = PruningAxiomEngine(board, memory_bank)
    gateway = DeepInteractionGateway(board, memory_bank)
    gateway.register(operator)
    reporter = MacroReportAgent(board, memory_bank)

    print("== Predictive interventions ==")
    pprint(healing.propose_interventions("incident-42"))

    print("\n== Operator recommendation ==")
    pprint(gateway.ask("ops-healer", "How should we stabilize the incident-42 cascade?"))

    memory_bank.decay(stale_axiom, amount=0.3)
    print("\n== Pruning result ==")
    pprint(pruning.prune(max_age_seconds=0.0))

    print("\n== Macro report ==")
    pprint(reporter.summarize(["incident-42", "worker-pool"]))


if __name__ == "__main__":
    main()
