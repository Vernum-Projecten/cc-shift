//! The OAuth calls Claude Code makes for a login: refreshing it and asking
//! for its usage.
//!
//! Anthropic does not document these. The values below are the ones the
//! Claude Code 2.1.300 binary uses (recorded on #25 and #9), so they can
//! change with any Claude Code update; a response in another form makes the
//! account's usage unknown, never a failure of `cs` as a whole.

use serde_json::{Value, json};

use crate::anthropic::http::{Request, Response};
use crate::error::Error;
use crate::login::Login;
use crate::store::state::Recorded;
use crate::usage::Window;

/// The token endpoint a refresh goes to.
pub const TOKEN_URL: &str = "https://platform.claude.com/v1/oauth/token";

/// Claude Code's OAuth client.
pub const CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";

/// The usage endpoint.
pub const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

/// The beta header Claude Code's OAuth calls carry.
const OAUTH_BETA: &str = "anthropic-beta: oauth-2025-04-20";

/// A refresh is due this many seconds before the access token expires.
pub const REFRESH_MARGIN_SECONDS: i64 = 10 * 60;

/// The fields of a stored login `cs` uses. Its debug output shows no token.
#[derive(Clone, PartialEq, Eq)]
pub struct Stored {
    access_token: String,
    refresh_token: String,
    /// When the access token expires, in Unix milliseconds.
    pub expires_at_ms: i64,
    scopes: Vec<String>,
}

impl std::fmt::Debug for Stored {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stored")
            .field("expires_at_ms", &self.expires_at_ms)
            .finish_non_exhaustive()
    }
}

impl Stored {
    /// Reads the fields `cs` uses from a stored login, the JSON object
    /// `claudeAiOauth`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Format`] when a field is missing or of another type.
    pub fn of(login: &Login) -> Result<Self, Error> {
        let value = parse(login.as_bytes(), "a stored login")?;
        let oauth = at(&value, "claudeAiOauth");
        let text = |key: &str| {
            at(oauth, key)
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| Error::Format(format!("a stored login has no {key}")))
        };
        Ok(Self {
            access_token: text("accessToken")?,
            refresh_token: text("refreshToken")?,
            expires_at_ms: at(oauth, "expiresAt")
                .as_i64()
                .ok_or_else(|| Error::Format("a stored login has no expiresAt".to_owned()))?,
            scopes: at(oauth, "scopes")
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Returns whether the access token expires within
    /// [`REFRESH_MARGIN_SECONDS`] of `now` (Unix seconds).
    #[must_use]
    pub fn expires_soon(&self, now: i64) -> bool {
        self.expires_at_ms.div_euclid(1000) - now < REFRESH_MARGIN_SECONDS
    }

    /// Returns the request that refreshes this login.
    #[must_use]
    pub fn refresh_request(&self) -> Request {
        Request {
            method: "POST",
            url: TOKEN_URL.to_owned(),
            headers: vec!["Content-Type: application/json".to_owned()],
            body: Some(
                json!({
                    "grant_type": "refresh_token",
                    "refresh_token": self.refresh_token,
                    "client_id": CLIENT_ID,
                    "scope": self.scopes.join(" "),
                })
                .to_string(),
            ),
        }
    }

    /// Returns the request that reads this login's usage.
    #[must_use]
    pub fn usage_request(&self) -> Request {
        Request {
            method: "GET",
            url: USAGE_URL.to_owned(),
            headers: vec![
                format!("Authorization: Bearer {}", self.access_token),
                OAUTH_BETA.to_owned(),
            ],
            body: None,
        }
    }
}

/// Applies a token response to a stored login and returns the new login.
///
/// Every field Claude Code keeps besides the tokens, their expiry times and
/// the scopes stays as it was.
///
/// # Errors
///
/// Returns [`Error::LoggedOut`] for a refused refresh (status 400 or 401),
/// [`Error::Http`] for another status, and [`Error::Format`] for a body in
/// another form.
pub fn apply_refresh(
    name: &str,
    login: &Login,
    response: &Response,
    now: i64,
) -> Result<Login, Error> {
    match response.status {
        200 => {}
        400 | 401 => return Err(Error::LoggedOut(name.to_owned())),
        other => {
            return Err(Error::Http(format!(
                "refreshing '{name}' answered with status {other}"
            )));
        }
    }
    let body = parse(&response.body, "a token response")?;
    let access = at(&body, "access_token")
        .as_str()
        .ok_or_else(|| Error::Format("a token response has no access_token".to_owned()))?;
    let expires_in = at(&body, "expires_in")
        .as_i64()
        .ok_or_else(|| Error::Format("a token response has no expires_in".to_owned()))?;
    let mut value = parse(login.as_bytes(), "a stored login")?;
    let oauth = value
        .get_mut("claudeAiOauth")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| Error::Format("a stored login has no claudeAiOauth".to_owned()))?;
    let now_ms = now.saturating_mul(1000);
    oauth.insert("accessToken".to_owned(), json!(access));
    oauth.insert(
        "expiresAt".to_owned(),
        json!(now_ms.saturating_add(expires_in.saturating_mul(1000))),
    );
    if let Some(refresh) = at(&body, "refresh_token").as_str() {
        oauth.insert("refreshToken".to_owned(), json!(refresh));
    }
    if let Some(seconds) = at(&body, "refresh_token_expires_in").as_i64() {
        oauth.insert(
            "refreshTokenExpiresAt".to_owned(),
            json!(now_ms.saturating_add(seconds.saturating_mul(1000))),
        );
    }
    if let Some(scope) = at(&body, "scope").as_str() {
        oauth.insert(
            "scopes".to_owned(),
            json!(scope.split_whitespace().collect::<Vec<_>>()),
        );
    }
    let bytes = serde_json::to_vec(&value)
        .map_err(|e| Error::Format(format!("cannot encode a stored login: {e}")))?;
    Ok(Login::from_bytes(bytes))
}

/// Reads both windows from a usage response.
///
/// A window that is missing or `null` is left out, and so is a window with
/// no reset time.
///
/// # Errors
///
/// Returns [`Error::LoggedOut`] for status 401, [`Error::Http`] for another
/// status other than 200, and [`Error::Format`] for a body in another form.
pub fn parse_usage(name: &str, response: &Response, now: i64) -> Result<Recorded, Error> {
    match response.status {
        200 => {}
        401 => return Err(Error::LoggedOut(name.to_owned())),
        other => {
            return Err(Error::Http(format!(
                "the usage of '{name}' answered with status {other}"
            )));
        }
    }
    let body = parse(&response.body, "a usage response")?;
    if !body.is_object() {
        return Err(Error::Format(
            "a usage response is not an object".to_owned(),
        ));
    }
    Ok(Recorded {
        five_hour: window(at(&body, "five_hour")),
        seven_day: window(at(&body, "seven_day")),
        read_at: Some(now),
    })
}

/// Reads one window: `utilization` from 0 to 100 and `resets_at` as an RFC
/// 3339 time.
fn window(value: &Value) -> Option<Window> {
    let used_percent = at(value, "utilization").as_f64()?;
    let resets_at = at(value, "resets_at")
        .as_str()?
        .parse::<jiff::Timestamp>()
        .ok()?
        .as_second();
    Some(Window {
        used_percent,
        resets_at,
    })
}

/// Returns a field of a JSON object, or `null` when it is missing.
fn at<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&Value::Null)
}

fn parse(bytes: &[u8], what: &str) -> Result<Value, Error> {
    // NOTE: no specification governs this: our own design. A syntax error
    // names a line and column, never the content, so it is safe to show.
    serde_json::from_slice(bytes).map_err(|e| Error::Format(format!("{what} is not JSON: {e}")))
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{Stored, apply_refresh, parse_usage};
    use crate::anthropic::http::Response;
    use crate::error::Error;
    use crate::login::Login;
    use serde_json::{Value, json};

    const NOW: i64 = 1_800_000_000;

    fn login(expires_at_ms: i64) -> Login {
        Login::from_bytes(
            json!({"claudeAiOauth": {
                "accessToken": "access-1",
                "refreshToken": "refresh-1",
                "expiresAt": expires_at_ms,
                "refreshTokenExpiresAt": 1,
                "scopes": ["user:inference", "user:profile"],
                "subscriptionType": "team",
                "rateLimitTier": "tier"
            }})
            .to_string()
            .into_bytes(),
        )
    }

    fn ok(body: &Value) -> Response {
        Response {
            status: 200,
            body: body.to_string().into_bytes(),
        }
    }

    #[test]
    fn a_token_close_to_expiry_is_due() -> Result<(), Error> {
        assert!(
            Stored::of(&login((NOW + 60) * 1000))?.expires_soon(NOW),
            "1 minute left"
        );
        assert!(
            !Stored::of(&login((NOW + 3600) * 1000))?.expires_soon(NOW),
            "1 hour left"
        );
        Ok(())
    }

    #[test]
    fn the_refresh_request_matches_claude_code() -> Result<(), Box<dyn std::error::Error>> {
        let request = Stored::of(&login(0))?.refresh_request();
        assert_eq!(request.url, "https://platform.claude.com/v1/oauth/token");
        let body: Value = serde_json::from_str(request.body.as_deref().unwrap_or_default())?;
        assert_eq!(
            body,
            json!({
                "grant_type": "refresh_token",
                "refresh_token": "refresh-1",
                "client_id": "9d1c250a-e61b-44d9-88ed-5944d1962f5e",
                "scope": "user:inference user:profile"
            })
        );
        Ok(())
    }

    #[test]
    fn a_refresh_keeps_every_other_field() -> Result<(), Box<dyn std::error::Error>> {
        let response = ok(&json!({
            "access_token": "access-2",
            "refresh_token": "refresh-2",
            "expires_in": 28_800,
            "scope": "user:inference"
        }));
        let new = apply_refresh("a", &login(0), &response, NOW)?;
        let value: Value = serde_json::from_slice(new.as_bytes())?;
        let oauth = &value["claudeAiOauth"];
        assert_eq!(oauth["accessToken"], "access-2");
        assert_eq!(oauth["refreshToken"], "refresh-2");
        assert_eq!(oauth["expiresAt"], json!((NOW + 28_800) * 1000));
        assert_eq!(oauth["scopes"], json!(["user:inference"]));
        assert_eq!(oauth["subscriptionType"], "team");
        assert_eq!(oauth["rateLimitTier"], "tier");
        assert_eq!(oauth["refreshTokenExpiresAt"], 1, "kept when not sent");
        Ok(())
    }

    #[test]
    fn a_refresh_without_a_new_refresh_token_keeps_the_old_one()
    -> Result<(), Box<dyn std::error::Error>> {
        let response = ok(&json!({"access_token": "access-2", "expires_in": 60}));
        let new = apply_refresh("a", &login(0), &response, NOW)?;
        let value: Value = serde_json::from_slice(new.as_bytes())?;
        assert_eq!(value["claudeAiOauth"]["refreshToken"], "refresh-1");
        Ok(())
    }

    #[test]
    fn a_refused_refresh_is_logged_out() {
        for status in [400, 401] {
            let response = Response {
                status,
                body: br#"{"error":"invalid_grant"}"#.to_vec(),
            };
            assert!(
                matches!(
                    apply_refresh("a", &login(0), &response, NOW),
                    Err(Error::LoggedOut(_))
                ),
                "status {status}"
            );
        }
    }

    #[test]
    fn a_server_error_is_not_logged_out() {
        let response = Response {
            status: 503,
            body: Vec::new(),
        };
        assert!(matches!(
            apply_refresh("a", &login(0), &response, NOW),
            Err(Error::Http(_))
        ));
    }

    #[test]
    fn usage_reads_both_windows() -> Result<(), Error> {
        let response = ok(&json!({
            "five_hour": {"utilization": 31.0, "resets_at": "2027-01-15T08:00:00.123456+00:00"},
            "seven_day": {"utilization": 26, "resets_at": "2027-01-20T00:00:00Z"},
            "seven_day_opus": null
        }));
        let recorded = parse_usage("a", &response, NOW)?;
        assert_eq!(
            recorded.five_hour.map(|w| (w.used_percent, w.resets_at)),
            Some((31.0, 1_800_000_000))
        );
        assert_eq!(recorded.seven_day.map(|w| w.resets_at), Some(1_800_403_200));
        assert_eq!(recorded.read_at, Some(NOW));
        Ok(())
    }

    #[test]
    fn a_window_without_a_reset_time_is_left_out() -> Result<(), Error> {
        let response = ok(&json!({
            "five_hour": {"utilization": 0.0, "resets_at": null},
            "seven_day": null
        }));
        let recorded = parse_usage("a", &response, NOW)?;
        assert_eq!((recorded.five_hour, recorded.seven_day), (None, None));
        Ok(())
    }

    #[test]
    fn debug_output_never_shows_a_token() -> Result<(), Error> {
        let stored = Stored::of(&login(0))?;
        assert!(!format!("{stored:?}").contains("access-1"), "{stored:?}");
        Ok(())
    }
}
