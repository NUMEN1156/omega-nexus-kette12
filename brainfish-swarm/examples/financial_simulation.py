from pprint import pprint

from brainfish import (
    DeepInteractionGateway,
    GraphRAGEnvironmentBoard,
    LLMMarketPersonaAgent,
    MacroReportAgent,
    SemanticReasoningBank,
)


def main() -> None:
    memory_bank = SemanticReasoningBank()
    board = GraphRAGEnvironmentBoard()

    memory_bank.remember("Liquidity contractions often propagate first into regional banks and small-cap credit.")
    memory_bank.remember("A softer CPI print can delay defensive rotation when earnings breadth improves.")
    memory_bank.remember("Energy shocks increase macro fragility and deserve elevated severity weighting.")

    board.add_entity("macro/cpi", kind="signal", severity=0.35, value="2.4%")
    board.add_entity("macro/oil", kind="signal", severity=0.75, value="+8%")
    board.add_entity("asset/NQ", kind="asset", severity=0.55)
    board.link("macro/cpi", "asset/NQ", relation="supports", weight=0.4)
    board.link("macro/oil", "asset/NQ", relation="pressures", weight=0.8)

    bull = LLMMarketPersonaAgent("athena-bull", "macro-momentum", memory_bank, board, persona="bull")
    bear = LLMMarketPersonaAgent("janus-bear", "systemic-risk", memory_bank, board, persona="bear")
    risk = LLMMarketPersonaAgent("sentinel", "portfolio-hedging", memory_bank, board, persona="risk_officer")

    gateway = DeepInteractionGateway(board, memory_bank)
    for agent in (bull, bear, risk):
        gateway.register(agent)

    signals = [
        "CPI cools faster than expected",
        "oil rallies on supply disruption",
        "tech earnings breadth remains positive",
    ]

    print("== Persona decisions ==")
    for agent in (bull, bear, risk):
        pprint(agent.evaluate_market("NQ", signals))

    print("\n== Swarm discussion ==")
    pprint(gateway.ask_all("What is the highest-probability macro cascade for NQ?"))

    print("\n== Macro report ==")
    pprint(MacroReportAgent(board, memory_bank).summarize(["macro/oil", "asset/NQ"]))


if __name__ == "__main__":
    main()
