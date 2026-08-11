//! Replay API: request parsing and budget validation live in [`request`],
//! response assembly in [`response`]. This module is the boundary between them.

mod request;
mod response;

pub(crate) use request::ReplayRequest;
pub(crate) use response::replay_json;

use request::{parse_replay_request, validate_replay_request};
use response::replay_error_json;

pub(crate) fn api_replay_response(query: &str) -> (&'static str, &'static str, String) {
    let request = parse_replay_request(query);
    match validate_replay_request(&request) {
        Ok(()) => (
            "200 OK",
            "application/json; charset=utf-8",
            replay_json(request),
        ),
        Err(error) => (
            "400 Bad Request",
            "application/json; charset=utf-8",
            replay_error_json(error),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::request::{MAX_EVOLVED_REPLAY_COST, ReplayRequestError};
    use super::{api_replay_response, parse_replay_request, validate_replay_request};

    #[test]
    fn oversized_evolved_replay_returns_bad_request() {
        let request =
            parse_replay_request("mode=evolved&generations=40&population=96&evaluation_steps=500");
        let cost = 40 * 96 * 500;

        assert_eq!(
            validate_replay_request(&request),
            Err(ReplayRequestError::EvolvedBudgetExceeded {
                cost,
                max_cost: MAX_EVOLVED_REPLAY_COST
            })
        );

        let (status, content_type, body) =
            api_replay_response("mode=evolved&generations=40&population=96&evaluation_steps=500");
        let value: serde_json::Value =
            serde_json::from_str(&body).expect("error should serialize as JSON");

        assert_eq!(status, "400 Bad Request");
        assert_eq!(content_type, "application/json; charset=utf-8");
        assert_eq!(
            value["error"],
            "evolved replay request exceeds the interactive preview budget"
        );
        assert_eq!(value["cost"].as_u64(), Some(cost as u64));
        assert_eq!(
            value["max_cost"].as_u64(),
            Some(MAX_EVOLVED_REPLAY_COST as u64)
        );
    }
}
