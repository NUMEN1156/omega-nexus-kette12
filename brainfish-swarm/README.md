# Brainfish (brainfish-swarm)

Ein dezentrales, selbstreparierendes und prädiktives Kognitionssystem nach dem Mirrorfish-Prinzip, das biologische Neuroplastizität, kybernetische Ultrastabilität und makroskopische Schwarmintelligenz vereint.

## Komponenten

- `brainfish/memory.py` — `SemanticReasoningBank` für vektorbasierte Ähnlichkeitssuche
- `brainfish/environment.py` — `GraphRAGEnvironmentBoard` als stigmergischer Wissensgraph
- `brainfish/engines.py` — prädiktive Heilung und axiomatisches Pruning
- `brainfish/agents.py` — dezentrale Agenten und LLM-Marktpersonas
- `brainfish/gateway.py` — Makroberichte und direkte Interaktion mit Personas

## Installation

```bash
pip install -r requirements.txt
```

Optional kann ein OpenAI-kompatibler Client als Callback an die Agenten übergeben werden. Ohne LLM-Zugriff arbeiten die Beispiele vollständig lokal und deterministisch.

## Schnellstart

```bash
export PYTHONPATH=.
python examples/devops_simulation.py
python examples/financial_simulation.py
```
