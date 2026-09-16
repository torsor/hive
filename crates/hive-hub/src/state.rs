use std::sync::Arc;

use chrono::Utc;
use hive_common::{HubConfig, HiveHome};
use hive_protocol::Fleet;
use hive_protocol::HubEvent;
use tokio::sync::{broadcast, RwLock};

pub struct HubState {
    pub home: HiveHome,
    pub cfg: HubConfig,
    pub fleet: RwLock<Fleet>,
    pub events: broadcast::Sender<HubEvent>,
    pub http: reqwest::Client,
}

impl HubState {
    pub fn new(
        home: HiveHome,
        cfg: HubConfig,
        events: broadcast::Sender<HubEvent>,
    ) -> Arc<Self> {
        Arc::new(Self {
            home,
            cfg,
            fleet: RwLock::new(Fleet {
                hosts: vec![],
                generated_at: Utc::now(),
            }),
            events,
            http: reqwest::Client::new(),
        })
    }
}
