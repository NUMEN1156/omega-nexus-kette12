import unittest

from Core.chainlink_recalibration import ChainlinkExternRecalibrator, NodeEvent, SovereignNode


def make_stable_node(index: int) -> SovereignNode:
    return SovereignNode(
        node_id=f"stable-{index}",
        status="STABLE",
        resonance_hz=117.02,
        feed_latency_ms=8.5,
        response_consistency=0.999,
        shield_active=True,
        entropy=0.97,
        yield_agst=109.29,
    )


class ChainlinkExternRecalibrationTest(unittest.TestCase):
    def setUp(self) -> None:
        self.recalibrator = ChainlinkExternRecalibrator()
        self.stable_nodes = [make_stable_node(index) for index in range(45)]
        self.alert_nodes = [
            SovereignNode(
                node_id="alert-1",
                status="ALERT",
                resonance_hz=116.70,
                feed_latency_ms=12.4,
                response_consistency=0.994,
                shield_active=True,
                entropy=0.98,
                yield_agst=108.9,
                recent_events=(
                    NodeEvent(day=1, summary="minor drift", severity=0.55),
                    NodeEvent(day=3, summary="alert spike", severity=0.75, kind="ALERT"),
                ),
            ),
            SovereignNode(
                node_id="alert-2",
                status="ALERT",
                resonance_hz=116.10,
                feed_latency_ms=14.1,
                response_consistency=0.991,
                shield_active=True,
                entropy=0.99,
                yield_agst=107.8,
                recent_events=(
                    NodeEvent(day=2, summary="alert spike", severity=0.82, kind="ALERT"),
                    NodeEvent(day=6, summary="consistency wobble", severity=0.62),
                ),
            ),
            SovereignNode(
                node_id="alert-3",
                status="ALERT",
                resonance_hz=116.10,
                feed_latency_ms=15.0,
                response_consistency=0.992,
                shield_active=False,
                entropy=1.01,
                yield_agst=107.5,
                recent_events=(
                    NodeEvent(day=1, summary="shield drop", severity=0.9, kind="ALERT"),
                    NodeEvent(day=4, summary="entropy rise", severity=0.8),
                    NodeEvent(day=7, summary="latency spike", severity=0.7),
                ),
            ),
            SovereignNode(
                node_id="alert-4",
                status="ALERT",
                resonance_hz=116.80,
                feed_latency_ms=11.3,
                response_consistency=0.996,
                shield_active=True,
                entropy=0.97,
                yield_agst=109.0,
                recent_events=(NodeEvent(day=5, summary="small drift", severity=0.4),),
            ),
            SovereignNode(
                node_id="alert-5",
                status="ALERT",
                resonance_hz=116.95,
                feed_latency_ms=10.2,
                response_consistency=0.997,
                shield_active=True,
                entropy=0.98,
                yield_agst=109.2,
                recent_events=(NodeEvent(day=2, summary="small drift", severity=0.45),),
            ),
        ]

    def test_build_plan_isolates_diagnoses_and_restores_alert_nodes(self) -> None:
        plan = self.recalibrator.build_plan(self.stable_nodes + self.alert_nodes)

        self.assertEqual(plan.isolated_node_ids, ("alert-1", "alert-2", "alert-3", "alert-4", "alert-5"))
        self.assertEqual(len(plan.diagnostics), 5)
        self.assertEqual(plan.priority_order, ("alert-3", "alert-2", "alert-1", "alert-4", "alert-5"))

        top_diagnostic = plan.diagnostics[0]
        self.assertEqual(top_diagnostic.node_id, "alert-3")
        self.assertAlmostEqual(top_diagnostic.resonance_drift_hz, 0.9)
        self.assertTrue(top_diagnostic.requires_shield_review)
        self.assertTrue(top_diagnostic.requires_entropy_review)
        self.assertEqual(top_diagnostic.recent_alert_event_count, 3)

        recalibrated = {node.node_id: node for node in plan.recalibrated_nodes}
        self.assertTrue(all(node.status == "STABLE" for node in recalibrated.values()))
        self.assertTrue(all(node.resonance_hz >= 117.0 for node in recalibrated.values()))
        self.assertTrue(all(node.shield_active for node in recalibrated.values()))
        self.assertTrue(all(node.entropy <= 0.98 for node in recalibrated.values()))
        self.assertTrue(all(node.yield_agst >= 109.29 for node in recalibrated.values()))

        self.assertEqual(plan.projected_stable_nodes, 50)
        self.assertGreaterEqual(plan.projected_average_resonance_hz, 117.0)
        self.assertLessEqual(plan.projected_average_entropy, 0.98)
        self.assertTrue(plan.success_target_met)

    def test_plan_adds_staged_steps_and_seven_day_monitoring(self) -> None:
        plan = self.recalibrator.build_plan(self.stable_nodes + self.alert_nodes, monitoring_days=7)

        self.assertEqual(len(plan.steps), 25)
        self.assertEqual(
            [step.phase for step in plan.steps[:5]],
            ["isolate", "diagnose", "fine_tune", "load_test", "reintegrate"],
        )
        self.assertEqual(plan.steps[0].day, 1)
        self.assertEqual(plan.steps[5].day, 2)
        self.assertEqual(plan.steps[10].day, 3)

        self.assertEqual(len(plan.monitoring_schedule), 35)
        first_snapshot = plan.monitoring_schedule[0]
        last_snapshot = plan.monitoring_schedule[-1]
        self.assertEqual(first_snapshot.day, 1)
        self.assertEqual(first_snapshot.node_id, "alert-3")
        self.assertEqual(last_snapshot.day, 7)
        self.assertEqual(last_snapshot.node_id, "alert-5")
        self.assertTrue(all(snapshot.status == "STABLE" for snapshot in plan.monitoring_schedule))


if __name__ == "__main__":
    unittest.main()
