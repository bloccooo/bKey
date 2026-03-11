use crate::{
    error::{Error, Result},
    types::{EnviDocument, Project},
};
use automerge::AutoCommit;
use autosurgeon::{hydrate, reconcile};
use uuid::Uuid;

pub fn add_project(doc: &mut AutoCommit, name: &str) -> Result<()> {
    let id = Uuid::now_v7().to_string();
    let mut state: EnviDocument = hydrate(doc)?;
    state.projects.insert(
        id.clone(),
        Project {
            id,
            name: name.to_string(),
            secret_ids: vec![],
        },
    );
    reconcile(doc, &state)?;
    Ok(())
}

pub fn remove_project(doc: &mut AutoCommit, id: &str) -> Result<()> {
    let mut state: EnviDocument = hydrate(doc)?;
    state.projects.remove(id);
    reconcile(doc, &state)?;
    Ok(())
}

pub fn update_project(doc: &mut AutoCommit, id: &str, name: &str) -> Result<()> {
    let mut state: EnviDocument = hydrate(doc)?;
    let project = state
        .projects
        .get_mut(id)
        .ok_or_else(|| Error::ProjectNotFound(id.to_string()))?;
    project.name = name.to_string();
    reconcile(doc, &state)?;
    Ok(())
}

pub fn set_project_secrets(
    doc: &mut AutoCommit,
    project_id: &str,
    secret_ids: Vec<String>,
) -> Result<()> {
    let mut state: EnviDocument = hydrate(doc)?;
    let project = state
        .projects
        .get_mut(project_id)
        .ok_or_else(|| Error::ProjectNotFound(project_id.to_string()))?;
    project.secret_ids = secret_ids;
    reconcile(doc, &state)?;
    Ok(())
}

pub fn list_projects(doc: &AutoCommit) -> Result<Vec<Project>> {
    let state: EnviDocument = hydrate(doc)?;
    Ok(state.projects.into_values().collect())
}
