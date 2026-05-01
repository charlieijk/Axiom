package dev.charlie.wayfinder.ai;

import java.util.List;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertTrue;

class DecisionEngineTest {
    private final DecisionEngine engine = new DecisionEngine();

    @Test
    void followsOwnerWhenAreaIsSafeAndOwnerIsFarAway() {
        DecisionResult result = engine.decide(snapshot(
                14.0,
                20.0,
                List.of(),
                List.of()
        ));

        assertEquals(DecisionAction.FOLLOW_OWNER, result.action());
        assertTrue(result.explanation().contains("Owner is 14.0 blocks away"));
    }

    @Test
    void avoidsLavaEvenWhenOwnerIsFarAway() {
        DecisionResult result = engine.decide(snapshot(
                14.0,
                20.0,
                List.of(new ObservedHazard(HazardType.LAVA, new Vector3(1.0, 64.0, 0.0), 1.0)),
                List.of()
        ));

        assertEquals(DecisionAction.AVOID_HAZARD, result.action());
        assertTrue(result.explanation().contains("Lava detected nearby"));
    }

    @Test
    void retreatsFromNearbyHostileMob() {
        DecisionResult result = engine.decide(snapshot(
                4.0,
                20.0,
                List.of(),
                List.of(new ObservedThreat("Zombie", new Vector3(2.0, 64.0, 0.0), 2.0))
        ));

        assertEquals(DecisionAction.RETREAT_FROM_MOB, result.action());
        assertTrue(result.explanation().contains("Zombie is nearby"));
    }

    @Test
    void holdsPositionWhenSafeAndOwnerIsClose() {
        DecisionResult result = engine.decide(snapshot(
                3.0,
                20.0,
                List.of(),
                List.of()
        ));

        assertEquals(DecisionAction.HOLD_POSITION, result.action());
        assertTrue(result.explanation().contains("Area is stable"));
    }

    @Test
    void lowHealthContributesToRetreatExplanation() {
        DecisionResult result = engine.decide(snapshot(
                3.0,
                5.0,
                List.of(),
                List.of()
        ));

        assertEquals(DecisionAction.RETREAT_FROM_MOB, result.action());
        assertTrue(result.explanation().contains("Health is low"));
    }

    @Test
    void detailedLineIncludesConfidenceAndTopScores() {
        DecisionResult result = engine.decide(snapshot(
                14.0,
                20.0,
                List.of(new ObservedHazard(HazardType.LAVA, new Vector3(1.0, 64.0, 0.0), 1.0)),
                List.of()
        ));

        assertTrue(result.detailedLine().contains("Confidence:"));
        assertTrue(result.detailedLine().contains("Scores:"));
        assertTrue(result.detailedLine().contains("Avoid Hazard="));
        assertEquals(DecisionAction.AVOID_HAZARD, result.evaluatedCandidates().getFirst().action());
    }

    @Test
    void hazardFallbackMovesAwayWhenNoSafeBlocksAreKnown() {
        DecisionResult result = engine.decide(snapshot(
                3.0,
                20.0,
                List.of(new ObservedHazard(HazardType.LAVA, new Vector3(1.0, 64.0, 0.0), 1.0)),
                List.of(),
                List.of()
        ));

        assertEquals(DecisionAction.AVOID_HAZARD, result.action());
        assertTrue(result.target().x() < 0.0);
        assertTrue(result.explanation().contains("No verified safe block found"));
    }

    @Test
    void lowHealthFallbackDoesNotInventThreatWhenNoSafeBlocksAreKnown() {
        DecisionResult result = engine.decide(snapshot(
                3.0,
                5.0,
                List.of(),
                List.of(),
                List.of()
        ));

        assertEquals(DecisionAction.RETREAT_FROM_MOB, result.action());
        assertTrue(result.explanation().contains("Health is low"));
        assertTrue(result.explanation().contains("scanning for recovery space"));
    }

    private static WorldSnapshot snapshot(
            double ownerDistance,
            double health,
            List<ObservedHazard> hazards,
            List<ObservedThreat> threats
    ) {
        return snapshot(
                ownerDistance,
                health,
                hazards,
                threats,
                List.of(
                        new CandidatePosition(new Vector3(4.0, 64.0, 4.0), 5.6, 7.0, 8.0, 8.0),
                        new CandidatePosition(new Vector3(-4.0, 64.0, -4.0), 5.6, 18.0, 12.0, 12.0)
                )
        );
    }

    private static WorldSnapshot snapshot(
            double ownerDistance,
            double health,
            List<ObservedHazard> hazards,
            List<ObservedThreat> threats,
            List<CandidatePosition> safePositions
    ) {
        Vector3 self = new Vector3(0.0, 64.0, 0.0);
        Vector3 owner = new Vector3(ownerDistance, 64.0, 0.0);

        return new WorldSnapshot(
                true,
                self,
                owner,
                ownerDistance,
                health,
                20.0,
                hazards,
                threats,
                safePositions
        );
    }
}
