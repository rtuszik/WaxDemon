use serde_json::{Value, json};

pub struct Driver {
    client: reqwest::Client,
    url: String,
}

impl Driver {
    pub async fn start() -> Self {
        let endpoint =
            std::env::var("WEBDRIVER_URL").expect("set WEBDRIVER_URL to the local ChromeDriver");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();
        let response:Value=client.post(format!("{endpoint}/session")).json(&json!({"capabilities":{"alwaysMatch":{"browserName":"chrome","goog:chromeOptions":{"args":["--headless=new","--no-sandbox","--disable-dev-shm-usage","--window-size=1440,1000","--host-resolver-rules=MAP www.discogs.com ~NOTFOUND"]},"goog:loggingPrefs":{"browser":"ALL"}}}})).send().await.unwrap().json().await.unwrap();
        let id = response["value"]["sessionId"]
            .as_str()
            .unwrap_or_else(|| panic!("ChromeDriver failed: {response}"));
        Self {
            client,
            url: format!("{endpoint}/session/{id}"),
        }
    }

    pub async fn post(&self, path: &str, body: Value) -> Value {
        let response = self
            .client
            .post(format!("{}{path}", self.url))
            .json(&body)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let result: Value = response.json().await.unwrap();
        assert!(status.is_success(), "WebDriver {path}: {result}");
        result["value"].clone()
    }

    pub async fn goto(&self, url: &str) {
        self.post("/url", json!({"url":url})).await;
    }

    pub async fn current_url(&self) -> String {
        let response: Value = self
            .client
            .get(format!("{}/url", self.url))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        response["value"].as_str().unwrap().to_string()
    }

    pub async fn script(&self, script: &str) -> Value {
        self.post("/execute/sync", json!({"script":script,"args":[]}))
            .await
    }

    pub async fn wait(&self, condition: &str) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            if self.script(&format!("return Boolean({condition})")).await == true {
                return;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Browser condition timed out: {condition}; logs: {}",
                self.post("/log", json!({"type":"browser"})).await
            );
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    pub async fn click(&self, selector: &str) {
        let element = self
            .post("/element", json!({"using":"css selector","value":selector}))
            .await;
        let id = element["element-6066-11e4-a52e-4f735466cecf"]
            .as_str()
            .unwrap();
        self.post(&format!("/element/{id}/click"), json!({})).await;
    }

    pub async fn close(self) {
        self.client
            .delete(self.url)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    pub async fn snapshot(&self, name: &str) {
        let Ok(directory) = std::env::var("BROWSER_ARTIFACTS") else {
            return;
        };
        let response: Value = self
            .client
            .get(format!("{}/screenshot", self.url))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            std::path::Path::new(&directory).join(format!("{name}.png.base64")),
            response["value"].as_str().unwrap(),
        )
        .unwrap();
    }
}
