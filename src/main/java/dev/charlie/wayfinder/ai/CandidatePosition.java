package dev.charlie.wayfinder.ai;

public record CandidatePosition(
        Vector3 position,
        double distanceToSelf,
        double distanceToOwner,
        double distanceToNearestHazard,
        double distanceToNearestThreat
) {
}

