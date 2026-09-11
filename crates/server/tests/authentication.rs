use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use secrecy::SecretBox;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{collections::BTreeMap, sync::Arc};
use time::{Duration, OffsetDateTime};
use tower::ServiceExt;
use tower_sessions::{
    SessionStore,
    session::{Id, Record},
};
use waxdemon_discogs::oauth::{OAuthClient, OAuthCredentials};
use waxdemon_server::auth::store::PgSessionStore;
use waxdemon_server::{auth::AuthState, credential_vault::CredentialVault};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(0);

#[tokio::test]
async fn public_health_probes_check_readiness_without_creating_sessions_or_exposing_errors() {
    let (pool, admin, schema) = database().await;
    let provider = MockServer::start().await;
    let mut browser = browser(&pool, &provider);
    for endpoint in ["/health/live", "/health/ready"] {
        let (status, headers, body) = browser.request("GET", endpoint, "", None).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(body.is_empty());
        assert!(!headers.contains_key("set-cookie"));
    }
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM app_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sessions, 0);
    pool.close().await;
    let (status, _, body) = browser.request("GET", "/health/ready", "", None).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.is_empty());
    assert_eq!(
        browser.request("GET", "/health/live", "", None).await.0,
        StatusCode::NO_CONTENT
    );
    cleanup(pool, admin, schema).await;
}

#[path = "support/webdriver.rs"]
mod webdriver;

#[tokio::test]
#[ignore = "requires built Leptos assets and WEBDRIVER_URL"]
async fn browser_hydration_library_settings_and_chart_lifecycle() {
    use serde_json::json;
    let (pool, admin, schema) = database().await;
    let provider_server = MockServer::start().await;
    provider(&provider_server, 700).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let vault = CredentialVault::new(
        "key".into(),
        BTreeMap::from([("key".into(), SecretBox::new(Box::new([7; 32])))]),
    )
    .unwrap();
    let oauth = OAuthClient::with_base(
        OAuthCredentials::new("consumer".into(), "consumer-secret".into()).unwrap(),
        &provider_server.uri(),
    )
    .unwrap();
    let app = AuthState::new(pool.clone(), oauth, Arc::new(vault), &origin)
        .unwrap()
        .router();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let driver = webdriver::Driver::start().await;
    driver.goto(&format!("{origin}/auth/login")).await;
    driver
        .wait("document.querySelector('form[action=\"/auth/login\"]')")
        .await;
    assert_original_styles(&driver).await;
    driver.snapshot("login").await;
    driver.click("form[action='/auth/login'] button").await;
    let authorization_url = driver.current_url().await;
    let login_logs = driver.post("/log", json!({"type":"browser"})).await;
    assert_eq!(
        authorization_url, "https://www.discogs.com/oauth/authorize?oauth_token=request-700",
        "{login_logs}"
    );
    driver
        .goto(&format!(
            "{origin}/auth/callback?oauth_token=request-700&oauth_verifier=verifier"
        ))
        .await;
    driver
        .wait("document.body.textContent.includes('Awaiting approval')")
        .await;
    assert_original_styles(&driver).await;
    driver.snapshot("pending").await;
    let id: i64 = sqlx::query_scalar(
        "UPDATE users SET status='approved',role='admin' WHERE discogs_id=700 RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO releases (id,artist,title,year,format,genres,styles) VALUES (1,'Browser Artist','First Record',2020,'Vinyl','[\"Jazz\"]','[]'),(2,'Another Artist','Second Record',2021,'CD','[\"Rock\"]','[]')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_collection_items (user_id,instance_id,release_id,added_date,condition,suggested_value,currency) VALUES ($1,10,1,'2025-01-01','Mint (M)',20.25,'EUR'),($1,11,2,'2025-01-02','Very Good (VG)',30,'USD')").bind(id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE user_collection_items SET condition=NULL,suggested_value=NULL,currency=NULL WHERE user_id=$1 AND instance_id=10").bind(id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_price_suggestions (user_id,release_id,condition,amount,currency,fetched_at) VALUES ($1,1,'Mint (M)',20.25,'EUR',now()),($1,1,'Very Good (VG)',10,'EUR',now())").bind(id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_collection_history (user_id,timestamp,total_items,value_min,value_median,value_max,currency) VALUES ($1,'2025-01-01T00:00:00Z',1,10,20,30,'EUR'),($1,'2025-02-01T00:00:00Z',2,15,25,35,'EUR')").bind(id).execute(&pool).await.unwrap();
    driver.goto(&origin).await;
    driver
        .wait("document.querySelectorAll('.chart canvas').length===5")
        .await;
    driver.snapshot("overview").await;
    assert_original_styles(&driver).await;
    driver.script("window.scrollTo(0,650)").await;
    driver.snapshot("history-chart").await;
    driver.script("window.scrollTo(0,0)").await;
    assert_eq!(driver.script("return echarts.getInstanceByDom(document.querySelector('.chart')).getOption().yAxis[0].name").await,"EUR");
    assert!(driver.script("return document.querySelector('.metrics').textContent.includes('Value (median)') && document.body.textContent.includes('Top valuable') && document.body.textContent.includes('Latest additions') && document.body.textContent.includes('Year distribution')").await.as_bool().unwrap());
    driver.script("window.testChart=echarts.getInstanceByDom(document.querySelector('.chart')); testChart.dispatchAction({type:'dataZoom',start:25,end:75}); document.querySelector('[aria-label=\"Chart currency\"]').dispatchEvent(new Event('change',{bubbles:true}));").await;
    driver.wait("testChart.getOption().series.length===3").await;
    assert_eq!(
        driver
            .script("return testChart.getOption().dataZoom[0].start")
            .await,
        25
    );
    driver.click("header nav a[href='/library']").await;
    driver
        .wait(
            "document.querySelectorAll('tbody tr').length===2 && !document.querySelector('.chart')",
        )
        .await;
    assert_eq!(driver.script("return testChart.isDisposed()").await, true);
    assert_original_styles(&driver).await;
    driver.snapshot("library-table").await;
    driver.wait("document.querySelector('tbody').textContent.includes('Mint (M) estimate') && document.querySelector('tbody').textContent.includes('20.25')").await;
    driver.click(".view-toggle button:nth-child(2)").await;
    driver
        .wait("document.querySelectorAll('.record-card').length===2")
        .await;
    driver.snapshot("library-grid").await;
    driver.script("const q=document.querySelector('input[name=q]'); q.value='First'; q.form.requestSubmit();").await;
    driver.wait("location.search.includes('q=First') && document.querySelectorAll('.record-card').length===1").await;
    driver.click(".record-card").await;
    driver
        .wait("document.querySelector('h1')?.textContent==='First Record'")
        .await;
    assert_original_styles(&driver).await;
    driver.snapshot("record-detail").await;
    driver.wait("document.querySelector('.detail-price').textContent.includes('20.25') && document.body.textContent.includes('Very Good (VG)')").await;
    driver.click("header nav a[href='/settings']").await;
    driver
        .wait("document.querySelector('[name=sync_interval_hours]')?.value==='24'")
        .await;
    assert_original_styles(&driver).await;
    driver.snapshot("settings").await;
    driver.script("const input=document.querySelector('[name=sync_interval_hours]');input.value='48';input.dispatchEvent(new Event('input',{bubbles:true}));input.form.requestSubmit();").await;
    driver
        .wait("document.body.textContent.includes('Preferences saved.')")
        .await;
    let interval: i32 =
        sqlx::query_scalar("SELECT sync_interval_hours FROM user_preferences WHERE user_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(interval, 48);
    driver.goto(&format!("{origin}/settings")).await;
    driver
        .wait("document.querySelector('[name=sync_interval_hours]')?.value==='48'")
        .await;
    provider(&provider_server, 701).await;
    let second = webdriver::Driver::start().await;
    second.goto(&format!("{origin}/auth/login")).await;
    second.click("form[action='/auth/login'] button").await;
    assert_eq!(
        second.current_url().await,
        "https://www.discogs.com/oauth/authorize?oauth_token=request-701"
    );
    second
        .goto(&format!(
            "{origin}/auth/callback?oauth_token=request-701&oauth_verifier=verifier"
        ))
        .await;
    second
        .wait("document.body.textContent.includes('Awaiting approval')")
        .await;
    second.goto(&format!("{origin}/library")).await;
    second.wait("document.body.textContent.includes('Awaiting approval') && !document.body.textContent.includes('First Record')").await;
    driver.click("header nav a[href='/admin/users']").await;
    driver
        .wait("document.querySelector('h1')?.textContent==='Pending accounts'")
        .await;
    assert_original_styles(&driver).await;
    driver.snapshot("approvals").await;
    driver
        .wait("document.querySelector('.approval')?.textContent.includes('owner-701')")
        .await;
    driver.click(".approval button").await;
    driver
        .wait("document.body.textContent.includes('No accounts are waiting for approval.')")
        .await;
    let second_id: i64 = sqlx::query_scalar(
        "SELECT id FROM users WHERE discogs_id=701 AND status='approved' AND role='user'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_sync_runs WHERE user_id=$1 AND status='queued'",
    )
    .bind(second_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(queued, 1);
    sqlx::query("INSERT INTO releases (id,artist,title) VALUES (3,'Private artist','Second user private record')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_collection_items (user_id,instance_id,release_id,added_date) VALUES ($1,10,3,'2025-01-03')").bind(second_id).execute(&pool).await.unwrap();
    second.click("form[method='get'] button").await;
    second
        .wait("document.querySelector('header nav a[href=\"/library\"]')")
        .await;
    second.click("header nav a[href='/library']").await;
    second.wait("document.querySelectorAll('tbody tr').length===1 && document.querySelector('tbody').textContent.includes('Second user private record')").await;
    assert_eq!(second.script("return document.body.textContent.includes('First Record') || !!document.querySelector('header nav a[href=\"/admin/users\"]')").await,false);
    second.click("tbody a").await;
    second
        .wait("document.querySelector('h1')?.textContent==='Second user private record'")
        .await;
    second.snapshot("second-user-record").await;
    second.close().await;
    driver.click("header nav a[href='/library']").await;
    driver
        .wait("document.querySelectorAll('tbody tr').length===2")
        .await;
    assert_eq!(
        driver
            .script("return document.body.textContent.includes('Second user private record')")
            .await,
        false
    );
    driver.click("header nav a[href='/']").await;
    driver
        .wait("document.querySelectorAll('.chart canvas').length===5")
        .await;
    driver.script("const c=echarts.getInstanceByDom(document.querySelector('[data-chart=\"Collection by genre; select a slice to filter the library\"]')); c.trigger('click',{name:'Jazz'});").await;
    driver
        .wait("location.search==='?genre=Jazz' && document.querySelectorAll('tbody tr').length===1")
        .await;
    driver
        .post("/goog/cdp/execute",json!({"cmd":"Emulation.setDeviceMetricsOverride","params":{"width":390,"height":844,"deviceScaleFactor":1,"mobile":true}}))
        .await;
    assert_eq!(driver.script("return window.innerWidth").await, 390);
    assert_eq!(
        driver
            .script("return document.documentElement.scrollWidth <= window.innerWidth")
            .await,
        true
    );
    driver.snapshot("library-mobile").await;
    let logs = driver.post("/log", json!({"type":"browser"})).await;
    let errors: Vec<_> = logs
        .as_array()
        .unwrap()
        .iter()
        .filter(|log| {
            log["level"] == "SEVERE"
                && !log["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("favicon.ico")
        })
        .collect();
    assert!(errors.is_empty(), "Browser errors: {errors:?}");
    driver.click("form[action='/auth/logout'] button").await;
    driver
        .wait("document.querySelector('form[action=\"/auth/login\"]')")
        .await;
    driver.close().await;
    server.abort();
    let _ = server.await;
    cleanup(pool, admin, schema).await;
}

async fn assert_original_styles(driver: &webdriver::Driver) {
    assert_eq!(driver.script("const body=getComputedStyle(document.body),panel=getComputedStyle(document.querySelector('.panel')),button=getComputedStyle(document.querySelector('button')),brand=getComputedStyle(document.querySelector('.brand')); return {background:body.backgroundColor,color:body.color,font:body.fontFamily.split(',')[0].trim(),padding:body.padding,lineHeight:body.lineHeight,panel:panel.backgroundColor,radius:panel.borderRadius,panelBorder:panel.borderTopWidth,panelPadding:panel.padding,button:button.backgroundColor,buttonRadius:button.borderRadius,brandSize:brand.fontSize,brandWeight:brand.fontWeight};").await, serde_json::json!({
        "background":"rgb(10, 10, 10)","color":"rgb(245, 245, 245)","font":"system-ui","padding":"24px","lineHeight":"22.4px",
        "panel":"rgb(23, 23, 23)","radius":"12px","panelBorder":"0px","panelPadding":"16px",
        "button":"rgb(38, 38, 38)","buttonRadius":"6px","brandSize":"24px","brandWeight":"600"
    }));
}

#[tokio::test]
async fn disconnect_stops_sync_reconnect_cannot_change_identity_and_deletion_removes_only_owner() {
    let (pool, admin, schema) = database().await;
    let server = MockServer::start().await;
    provider(&server, 41).await;
    let mut alice = browser(&pool, &server);
    alice.login().await;
    let info = alice.me().await;
    let id = info["user"]["id"].as_i64().unwrap();
    sqlx::query("UPDATE users SET status='approved' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let form = format!("csrf={}", info["csrf"].as_str().unwrap());
    alice
        .request("POST", "/api/sync", &form, Some("https://wax.example"))
        .await;
    assert_eq!(
        alice
            .request(
                "POST",
                "/auth/disconnect",
                &form,
                Some("https://evil.example")
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        alice
            .request(
                "POST",
                "/auth/disconnect",
                &form,
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    let connection: i64 =
        sqlx::query_scalar("SELECT count(*) FROM discogs_connections WHERE user_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(connection, 0);
    let run: String = sqlx::query_scalar("SELECT status FROM user_sync_runs WHERE user_id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(run, "cancelled");
    assert_eq!(alice.me().await["user"]["id"], id);
    assert_eq!(
        alice
            .request("POST", "/api/sync", &form, Some("https://wax.example"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    provider(&server, 42).await;
    assert_eq!(
        alice
            .request(
                "POST",
                "/auth/reconnect",
                &form,
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        alice
            .request(
                "GET",
                "/auth/callback?oauth_token=request-42&oauth_verifier=verifier",
                "",
                None
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    provider(&server, 41).await;
    assert_eq!(
        alice
            .request(
                "POST",
                "/auth/reconnect",
                &form,
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        alice
            .request(
                "GET",
                "/auth/callback?oauth_token=request-41&oauth_verifier=verifier",
                "",
                None
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    let info = alice.me().await;
    assert_eq!(info["user"]["id"], id);
    let connection: i64 =
        sqlx::query_scalar("SELECT count(*) FROM discogs_connections WHERE user_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(connection, 1);
    provider(&server, 42).await;
    let mut bob = browser(&pool, &server);
    bob.login().await;
    let other = bob.me().await["user"]["id"].as_i64().unwrap();
    let csrf = info["csrf"].as_str().unwrap();
    assert_eq!(
        alice
            .request(
                "POST",
                "/auth/delete-account",
                &format!("csrf={csrf}&confirm=wrong"),
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let cookie = alice.cookie.clone();
    assert_eq!(
        alice
            .request(
                "POST",
                "/auth/delete-account",
                &format!("csrf={csrf}&confirm=owner-41"),
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    let users: Vec<i64> = sqlx::query_scalar("SELECT id FROM users")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(users, vec![other]);
    alice.cookie = cookie;
    assert_eq!(
        alice.request("GET", "/auth/me", "", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(bob.me().await["user"]["id"], other);
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn deleting_legacy_owner_preserves_import_receipt_and_protects_last_admin() {
    let (pool, admin, schema) = database().await;
    let server = MockServer::start().await;
    provider(&server, 51).await;
    let mut owner = browser(&pool, &server);
    owner.login().await;
    let info = owner.me().await;
    let id = info["user"]["id"].as_i64().unwrap();
    sqlx::query("UPDATE users SET role='admin',status='approved' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO legacy_import (user_id,item_count,release_count,history_count,setting_count) VALUES ($1,1,1,1,1)").bind(id).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO collection_items (id,release_id,added_date) VALUES (1,1,'2025-01-01')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_stats_history (timestamp,total_items) VALUES ('2025-01-01',1)",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO settings (key,value) VALUES ('owner','legacy')")
        .execute(&pool)
        .await
        .unwrap();
    let form = format!("csrf={}&confirm=owner-51", info["csrf"].as_str().unwrap());
    assert_eq!(
        owner
            .request(
                "POST",
                "/auth/delete-account",
                &form,
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::CONFLICT
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_items")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    sqlx::query("INSERT INTO users (discogs_id,username,role,status) VALUES (52,'remaining-admin','admin','approved')").execute(&pool).await.unwrap();
    assert_eq!(
        owner
            .request(
                "POST",
                "/auth/delete-account",
                &form,
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    let receipt: Option<i64> = sqlx::query_scalar("SELECT user_id FROM legacy_import")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(receipt, None);
    let remaining:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM collection_items)+(SELECT count(*) FROM collection_stats_history)+(SELECT count(*) FROM settings)+(SELECT count(*) FROM app_sessions WHERE data->'axum-login.data'->>'user_id'=$1)").bind(id.to_string()).fetch_one(&pool).await.unwrap();
    assert_eq!(remaining, 0);
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn rendered_pages_escape_private_data_and_preserve_empty_filters_and_saved_preferences() {
    let (pool, admin, schema) = database().await;
    let server = MockServer::start().await;
    provider(&server, 61).await;
    let mut owner = browser(&pool, &server);
    owner.login().await;
    let id = owner.me().await["user"]["id"].as_i64().unwrap();
    sqlx::query("UPDATE users SET status='approved' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO releases (id,title,artist) VALUES (1,'</script><script>alert(1)</script>','Artist')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_collection_items (user_id,instance_id,release_id,added_date) VALUES ($1,1,1,'2025-01-01')").bind(id).execute(&pool).await.unwrap();
    let (status, headers, body) = owner
        .request(
            "GET",
            "/library?q=&year=&folder_id=&genre=&format=&condition=&currency=&sort=added_desc",
            "",
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Your library"));
    assert!(body.contains("\\u003c/script>"));
    assert!(!body.contains("<script>alert(1)</script>"));
    assert!(!body.contains("private-access"));
    let csp = headers["content-security-policy"].to_str().unwrap();
    let nonce = csp
        .split("'nonce-")
        .nth(1)
        .unwrap()
        .split('\'')
        .next()
        .unwrap();
    assert!(body.contains(&format!("nonce=\"{nonce}\"")));
    sqlx::query("INSERT INTO user_preferences (user_id,sync_interval_hours,price_refresh_hours,display_currency) VALUES ($1,48,72,'EUR')").bind(id).execute(&pool).await.unwrap();
    let (status, _, body) = owner.request("GET", "/settings", "", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("value=\"48\""));
    assert!(body.contains("value=\"72\""));
    assert!(body.contains("value=\"EUR\" selected"));
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn library_dashboard_and_settings_are_user_scoped_paginated_and_currency_aware() {
    let (pool, admin, schema) = database().await;
    let server = MockServer::start().await;
    provider(&server, 101).await;
    let mut alice = browser(&pool, &server);
    alice.login().await;
    let alice_info = alice.me().await;
    let alice_id = alice_info["user"]["id"].as_i64().unwrap();
    for route in [
        "/api/library",
        "/api/library/100",
        "/api/library/filters",
        "/api/dashboard",
        "/api/settings",
    ] {
        assert_eq!(
            alice.request("GET", route, "", None).await.0,
            StatusCode::FORBIDDEN,
            "{route}"
        );
    }
    provider(&server, 102).await;
    let mut bob = browser(&pool, &server);
    bob.login().await;
    let bob_id = bob.me().await["user"]["id"].as_i64().unwrap();
    sqlx::query("UPDATE users SET status='approved'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO releases (id,artist,title,year,format,genres,styles) VALUES (42,'Artist','100% Music',2020,'1 x Vinyl','[\"Jazz\"]','[]'),(43,'Other','Second',2021,'CD','[\"Rock\"]','[]')").execute(&pool).await.unwrap();
    for (user, instance, release, value, currency, note) in [
        (
            alice_id,
            100,
            42,
            "12.34567890123456789",
            "EUR",
            "Alice private",
        ),
        (alice_id, 101, 43, "3.21", "USD", "Alice second"),
        (bob_id, 100, 42, "999", "GBP", "Bob private"),
        (bob_id, 999, 43, "800", "GBP", "Bob only"),
    ] {
        sqlx::query("INSERT INTO user_collection_items (user_id,instance_id,release_id,added_date,folder_id,rating,notes,condition,suggested_value,currency) VALUES ($1,$2,$3,'2025-01-01T00:00:00Z',1,4,$6,'Mint (M)',$4::text::numeric,$5)")
            .bind(user).bind(instance as i64).bind(release as i64).bind(value).bind(currency).bind(note).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO user_collection_history (user_id,timestamp,total_items,value_median,currency) VALUES ($1,'2025-01-01T00:00:00Z',2,50,'EUR'),($2,'2025-01-01T00:00:00Z',2,999,'GBP')")
        .bind(alice_id).bind(bob_id).execute(&pool).await.unwrap();
    let (status, _, body) = alice
        .request("GET", "/api/library?page_size=1", "", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let page: waxdemon_core::library::LibraryPage = serde_json::from_str(&body).unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].instance_id, 101);
    assert!(!body.contains("Bob"));
    let (_, _, body) = alice
        .request("GET", "/api/library?page=2&page_size=1", "", None)
        .await;
    let page: waxdemon_core::library::LibraryPage = serde_json::from_str(&body).unwrap();
    assert_eq!(
        page.items[0].suggested_value.as_deref(),
        Some("12.34567890123456789")
    );
    assert_eq!(page.items[0].genres, vec!["Jazz"]);
    for query in [
        "q=100%25",
        "genre=Jazz",
        "year=2020",
        "format=Vinyl",
        "currency=EUR",
    ] {
        let (status, _, body) = alice
            .request("GET", &format!("/api/library?{query}"), "", None)
            .await;
        assert_eq!(status, StatusCode::OK, "{query}: {body}");
        let page: waxdemon_core::library::LibraryPage = serde_json::from_str(&body).unwrap();
        assert_eq!(page.total, 1, "{query}");
        assert_eq!(page.items[0].instance_id, 100);
    }
    for query in [
        "page=0",
        "page_size=0",
        "page_size=101",
        "sort=drop%20table",
    ] {
        assert_eq!(
            alice
                .request("GET", &format!("/api/library?{query}"), "", None)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        alice.request("GET", "/api/library/999", "", None).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, _, body) = alice.request("GET", "/api/library/100", "", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Alice private"));
    assert!(!body.contains("Bob"));
    let (status, _, body) = alice.request("GET", "/api/dashboard", "", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let dashboard: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(dashboard["total_items"], 2);
    assert_eq!(dashboard["values"].as_array().unwrap().len(), 2);
    assert_eq!(dashboard["history"][0]["median"], "50");
    assert!(!body.contains("GBP"));
    assert_eq!(dashboard["genres"].as_array().unwrap().len(), 2);
    assert_eq!(dashboard["summaries"][0]["median"], "50");
    assert_eq!(dashboard["summaries"][0]["average"], "25.0000000000000000");
    assert_eq!(dashboard["years"].as_array().unwrap().len(), 2);
    assert_eq!(dashboard["rankings"].as_array().unwrap().len(), 2);
    let (_, _, body) = alice
        .request("GET", "/api/dashboard?range=3m", "", None)
        .await;
    let recent: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(recent["summaries"][0]["median"], "50");
    assert!(recent["history"].as_array().unwrap().is_empty());
    sqlx::query("UPDATE user_collection_items SET condition=NULL,suggested_value=NULL,currency=NULL WHERE user_id=$1 AND instance_id=100").bind(alice_id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO user_price_suggestions (user_id,release_id,condition,amount,currency,fetched_at) VALUES ($1,42,'Mint (M)',40,'EUR',now()),($1,42,'Very Good (VG)',10,'EUR',now()),($2,42,'Mint (M)',999,'GBP',now())").bind(alice_id).bind(bob_id).execute(&pool).await.unwrap();
    let (status, _, body) = alice
        .request("GET", "/api/library?sort=price_desc&currency=EUR", "", None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let estimates: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(estimates["total"], 1);
    assert_eq!(estimates["items"][0]["suggested_value"], "40");
    assert_eq!(estimates["items"][0]["estimate_condition"], "Mint (M)");
    assert!(estimates["items"][0]["condition"].is_null());
    let (_, _, body) = alice.request("GET", "/api/library/100", "", None).await;
    let detail: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(detail["item"], estimates["items"][0]);
    let unchanged: bool = sqlx::query_scalar("SELECT condition IS NULL AND suggested_value IS NULL AND currency IS NULL FROM user_collection_items WHERE user_id=$1 AND instance_id=100").bind(alice_id).fetch_one(&pool).await.unwrap();
    assert!(unchanged);
    assert_eq!(detail["suggestions"].as_array().unwrap().len(), 2);
    let (_, _, body) = alice.request("GET", "/api/dashboard", "", None).await;
    let ranked: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(ranked["rankings"][0]["currency"], "EUR");
    assert_eq!(ranked["rankings"][0]["top"][0]["suggested_value"], "40");
    assert!(!body.contains("GBP"));
    sqlx::query("DELETE FROM user_price_suggestions WHERE user_id=$1 AND condition='Mint (M)'")
        .bind(alice_id)
        .execute(&pool)
        .await
        .unwrap();
    let (_, _, body) = alice.request("GET", "/api/library/100", "", None).await;
    let fallback: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(fallback["item"]["estimate_condition"], "Very Good (VG)");
    assert_eq!(fallback["item"]["suggested_value"], "10");
    sqlx::query("UPDATE user_collection_items SET condition='Good (G)' WHERE user_id=$1 AND instance_id=100").bind(alice_id).execute(&pool).await.unwrap();
    let (_, _, body) = alice.request("GET", "/api/library/100", "", None).await;
    let graded: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(graded["item"]["suggested_value"].is_null());
    assert!(graded["item"]["estimate_condition"].is_null());
    let grades = [
        "Mint (M)",
        "Near Mint (NM or M-)",
        "Very Good Plus (VG+)",
        "Very Good (VG)",
        "Good Plus (G+)",
        "Good (G)",
        "Fair (F)",
        "Poor (P)",
    ];
    for currency in ["USD", "EUR"] {
        for grade in grades.iter().rev() {
            sqlx::query("INSERT INTO user_price_suggestions (user_id,release_id,condition,amount,currency,fetched_at) VALUES ($1,42,$2,10,$3,now()) ON CONFLICT DO NOTHING").bind(alice_id).bind(grade).bind(currency).execute(&pool).await.unwrap();
        }
    }
    let (_, _, body) = alice.request("GET", "/api/library/100", "", None).await;
    let prices: serde_json::Value = serde_json::from_str(&body).unwrap();
    let ordered: Vec<_> = prices["suggestions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["currency"].as_str().unwrap(),
                p["condition"].as_str().unwrap(),
            )
        })
        .collect();
    let expected: Vec<_> = ["EUR", "USD"]
        .into_iter()
        .flat_map(|currency| grades.map(|grade| (currency, grade)))
        .collect();
    assert_eq!(ordered, expected);
    let (status, _, body) = alice.request("GET", "/api/library/filters", "", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("GBP"));
    let (_, _, body) = alice.request("GET", "/api/settings", "", None).await;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["preferences"]["sync_interval_hours"],
        24
    );
    let csrf = alice_info["csrf"].as_str().unwrap();
    let input =
        format!("csrf={csrf}&sync_interval_hours=0&price_refresh_hours=48&display_currency=EUR");
    assert_eq!(
        alice
            .request(
                "POST",
                "/api/settings",
                &input,
                Some("https://evil.example")
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        alice
            .request("POST", "/api/settings", &input, Some("https://wax.example"))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let invalid = format!("csrf={csrf}&sync_interval_hours=-1&price_refresh_hours=0");
    assert_eq!(
        alice
            .request(
                "POST",
                "/api/settings",
                &invalid,
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (_, _, body) = alice.request("GET", "/api/settings", "", None).await;
    let settings: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(settings["preferences"]["sync_interval_hours"], 0);
    assert_eq!(settings["preferences"]["display_currency"], "EUR");
    assert!(!body.contains("secret"));
    let (_, _, body) = bob.request("GET", "/api/settings", "", None).await;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["preferences"]["sync_interval_hours"],
        24
    );
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn sync_queue_enforces_approval_csrf_deduplication_daily_schedule_and_worker_recovery() {
    use waxdemon_server::jobs;
    let (pool, admin, schema) = database().await;
    jobs::setup(&pool).await.unwrap();
    jobs::setup(&pool).await.unwrap();
    waxdemon_db::run_migrations(&pool).await.unwrap();
    let server = MockServer::start().await;
    provider(&server, 77).await;
    let mut browser = browser(&pool, &server);
    assert_eq!(
        browser.request("GET", "/api/sync/status", "", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    browser.login().await;
    let info = browser.me().await;
    let id = info["user"]["id"].as_i64().unwrap();
    let form = format!("csrf={}", info["csrf"].as_str().unwrap());
    assert_eq!(
        browser
            .request("POST", "/api/sync", &form, Some("https://wax.example"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        browser.request("GET", "/api/sync/status", "", None).await.0,
        StatusCode::FORBIDDEN
    );
    sqlx::query("UPDATE users SET status='approved' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        browser
            .request(
                "POST",
                "/api/sync",
                "csrf=wrong",
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, _, body) = browser
        .request("POST", "/api/sync", &form, Some("https://wax.example"))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let run_id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["run_id"]
        .as_i64()
        .unwrap();
    let second = browser
        .request("POST", "/api/sync", &form, Some("https://wax.example"))
        .await
        .2;
    assert_eq!(body, second);
    assert_eq!(waxdemon_db::user_sync::enqueue_due(&pool).await.unwrap(), 0);
    assert_eq!(jobs::dispatch(&pool).await.unwrap(), 1);
    assert_eq!(jobs::dispatch(&pool).await.unwrap(), 0);
    let (task_id,payload):(String,serde_json::Value)=sqlx::query_as("SELECT id,job FROM apalis.jobs WHERE job->>'run_id'=$1 AND job->>'user_id'=$2 AND job_type=$3 AND status='Pending' ORDER BY run_at DESC LIMIT 1")
        .bind(run_id.to_string()).bind(id.to_string()).bind(jobs::NAMESPACE).fetch_one(&pool).await.unwrap();
    assert_eq!(payload, serde_json::json!({"run_id":run_id,"user_id":id}));
    let dead = format!("dead-{}", apalis::prelude::TaskId::new());
    sqlx::query("INSERT INTO apalis.workers (id,worker_type,storage_name,last_seen) VALUES ($1,$2,'test',now()-interval '10 minutes')")
        .bind(&dead).bind(jobs::NAMESPACE).execute(&pool).await.unwrap();
    sqlx::query("UPDATE apalis.jobs SET status='Running',lock_by=$2,lock_at=now()-interval '10 minutes' WHERE id=$1").bind(&task_id).bind(&dead).execute(&pool).await.unwrap();
    server.reset().await;
    Mock::given(path("/users/owner-77/collection/folders/0/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"pagination":{"page":1,"pages":1,"per_page":100,"items":0,"urls":{}},"releases":[]})))
        .mount(&server).await;
    for (suffix, body) in [
        ("fields", serde_json::json!({"fields":[]})),
        ("folders", serde_json::json!({"folders":[]})),
        (
            "value",
            serde_json::json!({"minimum":"€0.00","median":"€0.00","maximum":"€0.00"}),
        ),
    ] {
        Mock::given(path(format!("/users/owner-77/collection/{suffix}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;
    }
    let state = auth_state(&pool, &server);
    let worker = tokio::spawn(jobs::run(state));
    let completed = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        loop {
            let status: String =
                sqlx::query_scalar("SELECT status FROM user_sync_runs WHERE id=$1")
                    .bind(run_id)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            if status == "completed" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await;
    worker.abort();
    let _ = worker.await;
    completed.unwrap();
    assert_eq!(waxdemon_db::user_sync::enqueue_due(&pool).await.unwrap(), 0);
    sqlx::query("UPDATE user_sync_runs SET finished_at=now()-interval '25 hours' WHERE id=$1")
        .bind(run_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(waxdemon_db::user_sync::enqueue_due(&pool).await.unwrap(), 1);
    let (status, _, body) = browser.request("GET", "/api/sync/status", "", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body).unwrap()["run"]["status"],
        "queued"
    );
    let retry_run = serde_json::from_str::<serde_json::Value>(&body).unwrap()["run"]["id"]
        .as_i64()
        .unwrap();
    jobs::dispatch(&pool).await.unwrap();
    server.reset().await;
    Mock::given(path("/users/owner-77/collection/folders/0/releases"))
        .respond_with(ResponseTemplate::new(500).set_body_string("private-provider-error"))
        .expect(3)
        .mount(&server)
        .await;
    let worker = tokio::spawn(jobs::run(auth_state(&pool, &server)));
    let retried=tokio::time::timeout(std::time::Duration::from_secs(30),async {
        loop {
            let (status,attempts,error):(String,i32,Option<String>)=sqlx::query_as("SELECT status,attempts,error FROM user_sync_runs WHERE id=$1").bind(retry_run).fetch_one(&pool).await.unwrap();
            assert!(!error.as_deref().unwrap_or_default().contains("private-provider-error"));
            if status=="failed" {assert_eq!(attempts,3);break;}
            sqlx::query("UPDATE apalis.jobs j SET run_at=now()-interval '1 second' FROM user_sync_runs r WHERE r.id=$1 AND r.job_id=j.id AND j.status='Failed'")
                .bind(retry_run).execute(&pool).await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }).await;
    worker.abort();
    let _ = worker.await;
    retried.unwrap();
    sqlx::query("DELETE FROM apalis.jobs WHERE id IN (SELECT job_id FROM user_sync_runs)")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM apalis.jobs WHERE id=$1")
        .bind(task_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM apalis.workers WHERE id=$1")
        .bind(dead)
        .execute(&pool)
        .await
        .unwrap();
    cleanup(pool, admin, schema).await;
}

async fn database() -> (PgPool, PgPool, String) {
    let url = std::env::var("TEST_DATABASE_URL")
        .expect("authentication tests require a disposable TEST_DATABASE_URL");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let schema = format!(
        "auth_test_{}_{}",
        std::process::id(),
        NEXT_SCHEMA.fetch_add(1, Ordering::Relaxed)
    );
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let search_path = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(move |connection, _| {
            let path = search_path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false)")
                    .bind(path)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    waxdemon_db::run_migrations(&pool).await.unwrap();
    (pool, admin, schema)
}

async fn cleanup(pool: PgPool, admin: PgPool, schema: String) {
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

#[tokio::test]
async fn session_store_hashes_ids_preserves_expiry_and_does_not_resurrect_logout() {
    let (pool, admin, schema) = database().await;
    let store = PgSessionStore::new(pool.clone());
    let mut record = Record {
        id: Id::default(),
        data: HashMap::from([("example".into(), serde_json::json!(true))]),
        expiry_date: OffsetDateTime::now_utc() + Duration::days(90),
    };
    store.create(&mut record).await.unwrap();
    let (hash, data): (Vec<u8>, serde_json::Value) =
        sqlx::query_as("SELECT id_hash, data FROM app_sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(hash.len(), 32);
    assert!(!data.to_string().contains(&record.id.to_string()));
    let initial = store.load(&record.id).await.unwrap().unwrap();
    record.expiry_date += Duration::days(30);
    store.save(&record).await.unwrap();
    assert_eq!(
        store.load(&record.id).await.unwrap().unwrap().expiry_date,
        initial.expiry_date
    );
    store.delete(&record.id).await.unwrap();
    store.save(&record).await.unwrap();
    assert!(store.load(&record.id).await.unwrap().is_none());
    record.id = Id::default();
    record.expiry_date = OffsetDateTime::now_utc() - Duration::seconds(1);
    store.create(&mut record).await.unwrap();
    assert!(store.load(&record.id).await.unwrap().is_none());
    store.delete_expired().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM app_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    cleanup(pool, admin, schema).await;
}

#[derive(Clone)]
struct Browser {
    app: Router,
    cookie: Option<String>,
}

impl Browser {
    async fn request(
        &mut self,
        method: &str,
        uri: &str,
        body: &str,
        origin: Option<&str>,
    ) -> (StatusCode, HeaderMap, String) {
        let mut builder = Request::builder().method(method).uri(uri);
        if let Some(cookie) = &self.cookie {
            builder = builder.header("cookie", cookie);
        }
        if let Some(origin) = origin {
            builder = builder.header("origin", origin);
        }
        if method == "POST" {
            builder = builder.header("content-type", "application/x-www-form-urlencoded");
        }
        let response = self
            .app
            .clone()
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        if let Some(cookie) = headers.get("set-cookie") {
            self.cookie = Some(cookie.to_str().unwrap().split(';').next().unwrap().into());
        }
        let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
        (status, headers, String::from_utf8(bytes.to_vec()).unwrap())
    }

    async fn start(&mut self) -> (String, String) {
        let (status, _, page) = self.request("GET", "/auth/login", "", None).await;
        assert_eq!(status, StatusCode::OK);
        let csrf = page
            .split("value=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string();
        let old_cookie = self.cookie.clone().unwrap();
        let (status, headers, _) = self
            .request(
                "POST",
                "/auth/login",
                &format!("csrf={csrf}"),
                Some("https://wax.example"),
            )
            .await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        let authorize =
            url::Url::parse(headers.get("location").unwrap().to_str().unwrap()).unwrap();
        assert_eq!(authorize.host_str(), Some("www.discogs.com"));
        let token = authorize
            .query_pairs()
            .find(|(key, _)| key == "oauth_token")
            .unwrap()
            .1
            .into_owned();
        (
            format!("/auth/callback?oauth_token={token}&oauth_verifier=verifier"),
            old_cookie,
        )
    }

    async fn login(&mut self) -> (HeaderMap, String) {
        let (callback, old_cookie) = self.start().await;
        let (status, headers, body) = self.request("GET", &callback, "", None).await;
        assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
        assert_ne!(self.cookie.as_ref().unwrap(), &old_cookie);
        (headers, old_cookie)
    }

    async fn me(&mut self) -> serde_json::Value {
        let (status, headers, body) = self.request("GET", "/auth/me", "", None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        serde_json::from_str(&body).unwrap()
    }
}

fn browser(pool: &PgPool, server: &MockServer) -> Browser {
    Browser {
        app: auth_state(pool, server).router(),
        cookie: None,
    }
}

fn auth_state(pool: &PgPool, server: &MockServer) -> AuthState {
    let vault = CredentialVault::new(
        "key".into(),
        BTreeMap::from([("key".into(), SecretBox::new(Box::new([7; 32])))]),
    )
    .unwrap();
    let oauth = OAuthClient::with_base(
        OAuthCredentials::new("consumer".into(), "consumer-secret".into()).unwrap(),
        &server.uri(),
    )
    .unwrap();
    AuthState::new(pool.clone(), oauth, Arc::new(vault), "https://wax.example").unwrap()
}

async fn provider(server: &MockServer, discogs_id: i64) {
    server.reset().await;
    Mock::given(method("GET")).and(path("/oauth/request_token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("oauth_token=request-{discogs_id}&oauth_token_secret=request-secret&oauth_callback_confirmed=true")))
        .mount(server).await;
    Mock::given(method("POST"))
        .and(path("/oauth/access_token"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            "oauth_token=private-access-token&oauth_token_secret=private-access-secret",
        ))
        .mount(server)
        .await;
    Mock::given(path("/oauth/identity"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            serde_json::json!({"id": discogs_id, "username": format!("owner-{discogs_id}")}),
        ))
        .mount(server)
        .await;
}

#[tokio::test]
async fn callback_creates_pending_account_rotates_cookie_and_sets_absolute_ninety_days() {
    let (pool, admin, schema) = database().await;
    let server = MockServer::start().await;
    provider(&server, 42).await;
    let mut browser = browser(&pool, &server);
    let (headers, old_cookie) = browser.login().await;
    let cookie = headers.get("set-cookie").unwrap().to_str().unwrap();
    for expected in [
        "__Host-waxdemon.sid=",
        "HttpOnly",
        "SameSite=Lax",
        "Secure",
        "Path=/",
        "Max-Age=",
    ] {
        assert!(cookie.contains(expected), "{cookie}");
    }
    let max_age: i64 = cookie
        .split("Max-Age=")
        .nth(1)
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!((90 * 86400 - 5..=90 * 86400).contains(&max_age));
    let me = browser.me().await;
    assert_eq!(me["user"]["role"], "user");
    assert_eq!(me["user"]["status"], "pending");
    assert!(me["user"].get("session_revocation").is_none());
    let deadline = me["expires_at"].as_i64().unwrap();
    assert!(
        (90 * 86400 - 5..=90 * 86400)
            .contains(&(deadline - OffsetDateTime::now_utc().unix_timestamp()))
    );
    let later = browser.me().await;
    assert_eq!(later["expires_at"], me["expires_at"]);
    let mut old = browser.clone();
    old.cookie = Some(old_cookie);
    assert_eq!(
        old.request("GET", "/auth/me", "", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert!(
        browser
            .request("GET", "/", "", None)
            .await
            .2
            .contains("Awaiting approval")
    );
    assert_eq!(
        browser.request("GET", "/admin/users", "", None).await.0,
        StatusCode::FORBIDDEN
    );
    for uri in [
        "/api/dashboard-stats",
        "/api/collection/sync",
        "/api/collection/sync/status",
        "/api/image-proxy",
    ] {
        assert_eq!(
            browser.request("GET", uri, "", None).await.0,
            StatusCode::NOT_FOUND
        );
    }
    let session_data: String = sqlx::query_scalar("SELECT data::text FROM app_sessions LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!session_data.contains("private-access"));
    assert!(!session_data.contains("request-secret"));
    let connections: i64 = sqlx::query_scalar("SELECT count(*) FROM discogs_connections")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(connections, 1);
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn callbacks_are_browser_bound_expiring_single_use_and_provider_errors_are_redacted() {
    let (pool, admin, schema) = database().await;
    let server = MockServer::start().await;
    provider(&server, 42).await;
    let mut owner = browser(&pool, &server);
    let mut other = browser(&pool, &server);
    let (callback, _) = owner.start().await;
    assert_eq!(
        other.request("GET", &callback, "", None).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        owner
            .request(
                "GET",
                "/auth/callback?oauth_token=wrong&oauth_verifier=verifier",
                "",
                None
            )
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let stale = owner.clone();
    assert_eq!(
        owner.request("GET", &callback, "", None).await.0,
        StatusCode::SEE_OTHER
    );
    let mut replay = stale;
    assert_eq!(
        replay.request("GET", &callback, "", None).await.0,
        StatusCode::BAD_REQUEST
    );
    let exchanges = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.url.path() == "/oauth/access_token")
        .count();
    assert_eq!(exchanges, 1);
    provider(&server, 43).await;
    let mut expired = browser(&pool, &server);
    let (callback, _) = expired.start().await;
    sqlx::query("UPDATE oauth_attempts SET expires_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        expired.request("GET", &callback, "", None).await.0,
        StatusCode::BAD_REQUEST
    );
    assert!(
        !server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .any(|r| r.url.path() == "/oauth/access_token")
    );
    provider(&server, 44).await;
    let mut failed = browser(&pool, &server);
    let (callback, _) = failed.start().await;
    server.reset().await;
    Mock::given(path("/oauth/access_token"))
        .respond_with(ResponseTemplate::new(401).set_body_string("LEAK-provider-secret"))
        .expect(1)
        .mount(&server)
        .await;
    let (status, headers, body) = failed.request("GET", &callback, "", None).await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(!body.contains("LEAK"));
    assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
    assert_eq!(
        failed.request("GET", &callback, "", None).await.0,
        StatusCode::BAD_REQUEST
    );
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn only_existing_admin_can_approve_and_reject_and_repeat_login_cannot_promote() {
    let (pool, admin_db, schema) = database().await;
    sqlx::query("INSERT INTO users (discogs_id, username, role, status) VALUES (1, 'legacy-owner', 'admin', 'approved')").execute(&pool).await.unwrap();
    let server = MockServer::start().await;
    provider(&server, 1).await;
    let mut administrator = browser(&pool, &server);
    administrator.login().await;
    assert_eq!(administrator.me().await["user"]["role"], "admin");
    provider(&server, 2).await;
    let mut pending = browser(&pool, &server);
    pending.login().await;
    let pending_info = pending.me().await;
    let id = pending_info["user"]["id"].as_i64().unwrap();
    let url = format!("/admin/users/{id}/approve");
    let input = format!("csrf={}", pending_info["csrf"].as_str().unwrap());
    assert_eq!(
        pending
            .request("POST", &url, &input, Some("https://wax.example"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let csrf = administrator.me().await["csrf"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        administrator
            .request(
                "POST",
                &url,
                &format!("csrf={csrf}"),
                Some("https://evil.example")
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        administrator
            .request("POST", &url, "csrf=wrong", Some("https://wax.example"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        administrator
            .request(
                "POST",
                &url,
                &format!("csrf={csrf}"),
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    assert_eq!(pending.me().await["user"]["status"], "approved");
    let queued: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_sync_runs WHERE user_id=$1 AND status='queued'",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(queued, 1);
    assert_eq!(
        pending.request("GET", "/admin/users", "", None).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        administrator
            .request(
                "POST",
                &url,
                &format!("csrf={csrf}"),
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::CONFLICT
    );
    let mut relogin = browser(&pool, &server);
    relogin.login().await;
    assert_eq!(relogin.me().await["user"]["id"], pending_info["user"]["id"]);
    assert_eq!(relogin.me().await["user"]["role"], "user");
    provider(&server, 3).await;
    let mut rejected = browser(&pool, &server);
    rejected.login().await;
    let rejected_id = rejected.me().await["user"]["id"].as_i64().unwrap();
    assert_eq!(
        administrator
            .request(
                "POST",
                &format!("/admin/users/{rejected_id}/reject"),
                &format!("csrf={csrf}"),
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        rejected.request("GET", "/auth/me", "", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut retry = browser(&pool, &server);
    let (callback, _) = retry.start().await;
    assert_eq!(
        retry.request("GET", &callback, "", None).await.0,
        StatusCode::FORBIDDEN
    );
    let status: String = sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
        .bind(rejected_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "rejected");
    cleanup(pool, admin_db, schema).await;
}

#[tokio::test]
async fn logout_preserves_connection_and_revocation_disabled_and_expired_sessions_fail_closed() {
    let (pool, admin, schema) = database().await;
    let server = MockServer::start().await;
    provider(&server, 42).await;
    let mut first = browser(&pool, &server);
    first.login().await;
    let mut second = browser(&pool, &server);
    second.login().await;
    let csrf = first.me().await["csrf"].as_str().unwrap().to_string();
    assert_eq!(
        first
            .request("POST", "/auth/logout", &format!("csrf={csrf}"), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let mut old = first.clone();
    assert_eq!(
        first
            .request(
                "POST",
                "/auth/logout",
                &format!("csrf={csrf}"),
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        old.request("GET", "/auth/me", "", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let info = second.me().await;
    let connections: i64 = sqlx::query_scalar("SELECT count(*) FROM discogs_connections")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(connections, 1);
    let mut third = browser(&pool, &server);
    third.login().await;
    assert_eq!(
        second
            .request(
                "POST",
                "/auth/revoke-sessions",
                &format!("csrf={}", info["csrf"].as_str().unwrap()),
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::SEE_OTHER
    );
    assert_eq!(
        third.request("GET", "/auth/me", "", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut disabled = browser(&pool, &server);
    disabled.login().await;
    sqlx::query("UPDATE users SET status = 'disabled' WHERE discogs_id = 42")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        disabled.request("GET", "/auth/me", "", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut blocked = browser(&pool, &server);
    let (callback, _) = blocked.start().await;
    assert_eq!(
        blocked.request("GET", &callback, "", None).await.0,
        StatusCode::FORBIDDEN
    );
    provider(&server, 43).await;
    let mut expired = browser(&pool, &server);
    expired.login().await;
    sqlx::query("UPDATE app_sessions SET expires_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        expired.request("GET", "/auth/me", "", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    cleanup(pool, admin, schema).await;
}

#[tokio::test]
async fn login_start_requires_csrf_and_does_not_send_credentials_on_get() {
    let (pool, admin, schema) = database().await;
    let server = MockServer::start().await;
    let mut browser = browser(&pool, &server);
    let (status, headers, page) = browser.request("GET", "/auth/login", "", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers.get("referrer-policy").unwrap(), "same-origin");
    let csrf = page
        .split("value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    for origin in [None, Some("null"), Some("https://attacker.example")] {
        assert_eq!(
            browser
                .request("POST", "/auth/login", &format!("csrf={csrf}"), origin)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(
        browser
            .request(
                "POST",
                "/auth/login",
                "csrf=wrong",
                Some("https://wax.example")
            )
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert!(server.received_requests().await.unwrap().is_empty());
    cleanup(pool, admin, schema).await;
}
