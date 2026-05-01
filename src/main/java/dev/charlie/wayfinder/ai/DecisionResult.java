package dev.charlie.wayfinder.ai;

import java.util.List;

public record DecisionResult(
        DecisionGoal goal,
        DecisionAction action,
        double score,
        double confidence,
        Vector3 target,
        String explanation,
        List<DecisionCandidate> evaluatedCandidates
) {
    public DecisionResult {
        evaluatedCandidates = List.copyOf(evaluatedCandidates);
    }

    public String chatLine() {
        return "[Wayfinder] Goal: " + goal.displayName()
                + " | Action: " + action.displayName()
                + " | Reason: " + explanation;
    }

    public String detailedLine() {
        return chatLine()
                + " | Confidence: " + percent(confidence)
                + " | Scores: " + scoreSummary();
    }

    private String scoreSummary() {
        StringBuilder summary = new StringBuilder();
        int count = Math.min(3, evaluatedCandidates.size());
        for (int i = 0; i < count; i++) {
            DecisionCandidate candidate = evaluatedCandidates.get(i);
            if (!summary.isEmpty()) {
                summary.append(", ");
            }
            summary.append(candidate.action().displayName())
                    .append("=")
                    .append(Math.round(candidate.score()));
        }
        return summary.toString();
    }

    private static String percent(double value) {
        return Math.round(value * 100.0) + "%";
    }
}
