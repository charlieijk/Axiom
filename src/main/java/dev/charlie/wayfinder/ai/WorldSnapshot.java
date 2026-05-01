package dev.charlie.wayfinder.ai;

import java.util.Comparator;
import java.util.List;
import java.util.Optional;

public record WorldSnapshot(
        boolean hasOwner,
        Vector3 selfPosition,
        Vector3 ownerPosition,
        double ownerDistance,
        double health,
        double maxHealth,
        List<ObservedHazard> hazards,
        List<ObservedThreat> threats,
        List<CandidatePosition> safePositions
) {
    public WorldSnapshot {
        hazards = List.copyOf(hazards);
        threats = List.copyOf(threats);
        safePositions = List.copyOf(safePositions);
    }

    public double healthRatio() {
        if (maxHealth <= 0.0) {
            return 1.0;
        }
        return health / maxHealth;
    }

    public Optional<ObservedHazard> nearestHazard() {
        return hazards.stream().min(Comparator.comparingDouble(ObservedHazard::distance));
    }

    public Optional<ObservedThreat> nearestThreat() {
        return threats.stream().min(Comparator.comparingDouble(ObservedThreat::distance));
    }
}

