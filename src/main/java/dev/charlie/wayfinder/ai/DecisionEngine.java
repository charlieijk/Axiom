package dev.charlie.wayfinder.ai;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

public final class DecisionEngine {
    public static final double FOLLOW_DISTANCE = 7.0;
    public static final double LOW_HEALTH_RATIO = 0.35;

    public DecisionResult decide(WorldSnapshot snapshot) {
        List<DecisionCandidate> candidates = new ArrayList<>();

        snapshot.nearestHazard().ifPresent(hazard -> candidates.add(avoidHazard(snapshot, hazard)));
        if (!snapshot.threats().isEmpty() || snapshot.healthRatio() <= LOW_HEALTH_RATIO) {
            candidates.add(retreatFromMob(snapshot));
        }
        if (snapshot.hasOwner() && snapshot.ownerDistance() > FOLLOW_DISTANCE) {
            candidates.add(followOwner(snapshot));
        }
        candidates.add(holdPosition(snapshot));

        List<DecisionCandidate> sorted = candidates.stream()
                .sorted(Comparator.comparingDouble(DecisionCandidate::score).reversed())
                .toList();

        DecisionCandidate winner = sorted.getFirst();
        double secondScore = sorted.size() > 1 ? sorted.get(1).score() : 0.0;
        double confidence = clamp((winner.score() - secondScore + 40.0) / 100.0, 0.0, 1.0);

        return new DecisionResult(
                winner.goal(),
                winner.action(),
                winner.score(),
                confidence,
                winner.target(),
                explanationFor(winner),
                sorted
        );
    }

    private DecisionCandidate avoidHazard(WorldSnapshot snapshot, ObservedHazard hazard) {
        boolean hasVerifiedSafePosition = !snapshot.safePositions().isEmpty();
        CandidatePosition target = bestSafePosition(snapshot, true, fallbackAwayFromHazard(snapshot, hazard));
        double score = 130.0
                + hazard.severity() * 8.0
                + Math.max(0.0, 5.0 - hazard.distance()) * 5.0
                + target.distanceToNearestHazard();

        return new DecisionCandidate(
                DecisionGoal.STAY_ALIVE,
                DecisionAction.AVOID_HAZARD,
                score,
                target.position(),
                List.of(
                        new ReasonFactor("hazard", hazard.severity(), hazard.displayName() + " detected nearby."),
                        new ReasonFactor(
                                "safe_position",
                                target.distanceToNearestHazard(),
                                hasVerifiedSafePosition
                                        ? "Moving to a safer block away from the hazard."
                                        : "No verified safe block found yet; moving away from the hazard vector."
                        )
                )
        );
    }

    private DecisionCandidate retreatFromMob(WorldSnapshot snapshot) {
        boolean hasVerifiedSafePosition = !snapshot.safePositions().isEmpty();
        boolean hasThreat = snapshot.nearestThreat().isPresent();
        CandidatePosition target = bestSafePosition(snapshot, false, fallbackAwayFromThreat(snapshot));
        List<ReasonFactor> factors = new ArrayList<>();
        double score = 95.0;

        snapshot.nearestThreat().ifPresent(threat -> factors.add(new ReasonFactor(
                "threat",
                Math.max(1.0, 10.0 - threat.distance()),
                threat.displayName() + " is nearby."
        )));

        if (snapshot.healthRatio() <= LOW_HEALTH_RATIO) {
            factors.add(new ReasonFactor("low_health", snapshot.healthRatio(), "Health is low."));
            score += 25.0;
        }
        if (!snapshot.threats().isEmpty()) {
            score += Math.max(0.0, 10.0 - snapshot.nearestThreat().orElseThrow().distance()) * 3.0;
        }

        factors.add(new ReasonFactor(
                "retreat_position",
                target.distanceToNearestThreat(),
                hasVerifiedSafePosition
                        ? "Moving toward the safest nearby retreat position."
                        : hasThreat
                                ? "No verified retreat block found yet; moving away from the threat vector."
                                : "No verified retreat block found yet; holding position while scanning for recovery space."
        ));

        return new DecisionCandidate(
                DecisionGoal.STAY_ALIVE,
                DecisionAction.RETREAT_FROM_MOB,
                score,
                target.position(),
                factors
        );
    }

    private DecisionCandidate followOwner(WorldSnapshot snapshot) {
        double score = 55.0 + snapshot.ownerDistance() * 2.0;
        return new DecisionCandidate(
                DecisionGoal.FOLLOW_PLAYER,
                DecisionAction.FOLLOW_OWNER,
                score,
                snapshot.ownerPosition(),
                List.of(new ReasonFactor(
                        "owner_distance",
                        snapshot.ownerDistance(),
                        "Owner is " + rounded(snapshot.ownerDistance()) + " blocks away."
                ))
        );
    }

    private DecisionCandidate holdPosition(WorldSnapshot snapshot) {
        double score = snapshot.hasOwner() && snapshot.ownerDistance() <= FOLLOW_DISTANCE ? 35.0 : 10.0;
        return new DecisionCandidate(
                DecisionGoal.MAINTAIN_POSITION,
                DecisionAction.HOLD_POSITION,
                score,
                snapshot.selfPosition(),
                List.of(new ReasonFactor("stable_area", score, "Area is stable and the owner is close enough."))
        );
    }

    private CandidatePosition bestSafePosition(WorldSnapshot snapshot, boolean preferHazardDistance, Vector3 fallbackPosition) {
        return snapshot.safePositions().stream()
                .max(Comparator.comparingDouble(candidate -> safetyScore(candidate, preferHazardDistance)))
                .orElse(new CandidatePosition(
                        fallbackPosition,
                        snapshot.selfPosition().distanceTo(fallbackPosition),
                        snapshot.ownerPosition().distanceTo(fallbackPosition),
                        nearestHazardDistance(snapshot, fallbackPosition),
                        nearestThreatDistance(snapshot, fallbackPosition)
                ));
    }

    private Vector3 fallbackAwayFromHazard(WorldSnapshot snapshot, ObservedHazard hazard) {
        return moveAway(snapshot.selfPosition(), hazard.position(), 4.0);
    }

    private Vector3 fallbackAwayFromThreat(WorldSnapshot snapshot) {
        return snapshot.nearestThreat()
                .map(threat -> moveAway(snapshot.selfPosition(), threat.position(), 4.0))
                .orElse(snapshot.selfPosition());
    }

    private Vector3 moveAway(Vector3 self, Vector3 risk, double distance) {
        double dx = self.x() - risk.x();
        double dz = self.z() - risk.z();
        double length = Math.sqrt(dx * dx + dz * dz);
        if (length < 0.001) {
            dx = 1.0;
            dz = 0.0;
            length = 1.0;
        }
        return new Vector3(
                self.x() + dx / length * distance,
                self.y(),
                self.z() + dz / length * distance
        );
    }

    private double nearestHazardDistance(WorldSnapshot snapshot, Vector3 position) {
        return snapshot.hazards().stream()
                .mapToDouble(hazard -> hazard.position().distanceTo(position))
                .min()
                .orElse(12.0);
    }

    private double nearestThreatDistance(WorldSnapshot snapshot, Vector3 position) {
        return snapshot.threats().stream()
                .mapToDouble(threat -> threat.position().distanceTo(position))
                .min()
                .orElse(12.0);
    }

    private double safetyScore(CandidatePosition candidate, boolean preferHazardDistance) {
        double hazardWeight = preferHazardDistance ? 4.0 : 2.0;
        double threatWeight = preferHazardDistance ? 2.0 : 4.0;
        return candidate.distanceToNearestHazard() * hazardWeight
                + candidate.distanceToNearestThreat() * threatWeight
                - candidate.distanceToSelf() * 0.35
                - candidate.distanceToOwner() * 0.15;
    }

    private String explanationFor(DecisionCandidate candidate) {
        StringBuilder explanation = new StringBuilder();
        for (ReasonFactor factor : candidate.reasonFactors()) {
            if (!explanation.isEmpty()) {
                explanation.append(' ');
            }
            explanation.append(factor.explanation());
        }
        return explanation.toString();
    }

    private static double clamp(double value, double min, double max) {
        return Math.max(min, Math.min(max, value));
    }

    private static String rounded(double value) {
        return String.format("%.1f", value);
    }
}
