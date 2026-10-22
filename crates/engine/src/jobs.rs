use dbcraft_core::{QueryResult, SchemaNode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type JobId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Connect,
    Test,
    Schema,
    Query,
    Preview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum JobStatus {
    Pending { id: JobId, kind: JobKind },
    Running { id: JobId, kind: JobKind },
    Done {
        id: JobId,
        kind: JobKind,
        #[serde(skip_serializing_if = "Option::is_none")]
        query: Option<QueryResult>,
        #[serde(skip_serializing_if = "Option::is_none")]
        schema: Option<Vec<SchemaNode>>,
        message: String,
    },
    Failed { id: JobId, kind: JobKind, error: String },
}

impl JobStatus {
    pub fn id(&self) -> JobId {
        match self {
            Self::Pending { id, .. }
            | Self::Running { id, .. }
            | Self::Done { id, .. }
            | Self::Failed { id, .. } => *id,
        }
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Done { .. } | Self::Failed { .. })
    }
}
