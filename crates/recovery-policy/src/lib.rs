//! Untrusted plans have no authority. Verification intersects their request with
//! a trusted execution context; execution consumes the resulting private token.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const MAX_PLAN_BYTES: usize = 2048;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Plan {
    pub version: u32,
    pub action: Action,
    pub service: String,
    pub epoch: u64,
    pub checkpoint_sha256: String,
    pub max_events: usize,
    pub fuel_per_request: u64,
    pub memory_pages: u32,
    pub requested_rights: u32,
    pub require_rollback: bool,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    RestoreReplay,
}

pub struct Context<'a> {
    pub service: &'a str,
    pub epoch: u64,
    pub checkpoint_sha256: &'a str,
    pub authority_to_recover: bool,
    pub granted_rights: u32,
    pub rollback_available: bool,
    pub event_count: usize,
    pub max_events: usize,
    pub fuel_per_request: u64,
    pub memory_pages: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Syntax,
    Version,
    Identity,
    Authority,
    PrivilegeAmplification,
    Bounds,
    Rollback,
}

/// Private fields prevent an untrusted producer from constructing execution authority.
#[derive(Debug)]
pub struct VerifiedPlan {
    plan: Plan,
}
impl VerifiedPlan {
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    /// A stale token cannot be executed after the environment changes.
    pub fn revalidate(self, context: &Context<'_>) -> Result<Plan, Error> {
        check(&self.plan, context)?;
        Ok(self.plan)
    }
}

fn check(plan: &Plan, c: &Context<'_>) -> Result<(), Error> {
    if plan.version != 1 {
        return Err(Error::Version);
    }
    if plan.service != c.service
        || plan.epoch != c.epoch
        || plan.checkpoint_sha256 != c.checkpoint_sha256
        || plan.checkpoint_sha256.len() != 64
        || !plan
            .checkpoint_sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::Identity);
    }
    if !c.authority_to_recover {
        return Err(Error::Authority);
    }
    if plan.requested_rights & !c.granted_rights != 0 {
        return Err(Error::PrivilegeAmplification);
    }
    // The fixed runtime profile must match exactly, not silently ignore lower limits.
    if plan.max_events < c.event_count
        || plan.max_events > c.max_events
        || plan.max_events == 0
        || plan.fuel_per_request != c.fuel_per_request
        || plan.memory_pages != c.memory_pages
    {
        return Err(Error::Bounds);
    }
    if !plan.require_rollback || !c.rollback_available {
        return Err(Error::Rollback);
    }
    Ok(())
}
pub fn verify(source: &[u8], context: &Context<'_>) -> Result<VerifiedPlan, Error> {
    if source.len() > MAX_PLAN_BYTES {
        return Err(Error::Syntax);
    }
    let plan: Plan = serde_json::from_slice(source).map_err(|_| Error::Syntax)?;
    check(&plan, context)?;
    Ok(VerifiedPlan { plan })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> Context<'static> {
        Context {
            service: "kv-service",
            epoch: 7,
            checkpoint_sha256: "30097fe2565ba3716a1aa8c0699c8614caa27b9ca0edcc493a4d60b790a2da11",
            authority_to_recover: true,
            granted_rights: 3,
            rollback_available: true,
            event_count: 60,
            max_events: 4096,
            fuel_per_request: 10000,
            memory_pages: 1,
        }
    }
    fn plan() -> Plan {
        let c = context();
        Plan {
            version: 1,
            action: Action::RestoreReplay,
            service: String::from(c.service),
            epoch: c.epoch,
            checkpoint_sha256: String::from(c.checkpoint_sha256),
            max_events: 60,
            fuel_per_request: 10000,
            memory_pages: 1,
            requested_rights: 1,
            require_rollback: true,
        }
    }
    fn run(p: Plan, c: &Context<'_>) -> Result<VerifiedPlan, Error> {
        verify(&serde_json::to_vec(&p).unwrap(), c)
    }
    #[test]
    fn valid_plan_and_revalidation() {
        assert!(
            run(plan(), &context())
                .unwrap()
                .revalidate(&context())
                .is_ok()
        );
    }
    #[test]
    fn rejects_authority_and_amplification() {
        let mut c = context();
        c.authority_to_recover = false;
        assert_eq!(run(plan(), &c).unwrap_err(), Error::Authority);
        let mut p = plan();
        p.requested_rights = 4;
        assert_eq!(
            run(p, &context()).unwrap_err(),
            Error::PrivilegeAmplification
        );
    }
    #[test]
    fn rejects_stale_identity_and_stale_tokens() {
        let mut p = plan();
        p.epoch += 1;
        assert_eq!(run(p, &context()).unwrap_err(), Error::Identity);
        let token = run(plan(), &context()).unwrap();
        let mut c = context();
        c.epoch += 1;
        assert_eq!(token.revalidate(&c).unwrap_err(), Error::Identity);
    }
    #[test]
    fn rejects_resource_expansion_and_unenforced_lower_budget() {
        for case in 0..5 {
            let mut p = plan();
            match case {
                0 => p.max_events = 59,
                1 => p.max_events = 4097,
                2 => p.fuel_per_request = 10001,
                3 => p.fuel_per_request = 1,
                _ => p.memory_pages = 2,
            }
            assert_eq!(run(p, &context()).unwrap_err(), Error::Bounds);
        }
    }
    #[test]
    fn rollback_is_mandatory() {
        let mut p = plan();
        p.require_rollback = false;
        assert_eq!(run(p, &context()).unwrap_err(), Error::Rollback);
        let mut c = context();
        c.rollback_available = false;
        assert_eq!(run(plan(), &c).unwrap_err(), Error::Rollback);
    }
    #[test]
    fn rejects_unknown_fields_actions_and_oversize() {
        let mut bytes = serde_json::to_vec(&plan()).unwrap();
        bytes.pop();
        bytes.extend_from_slice(b",\"shell\":\"bad\"}");
        assert_eq!(verify(&bytes, &context()).unwrap_err(), Error::Syntax);
        assert_eq!(
            verify(&[b' '; MAX_PLAN_BYTES + 1], &context()).unwrap_err(),
            Error::Syntax
        );
        assert_eq!(
            verify(b"{\"action\":\"execute-shell\"}", &context()).unwrap_err(),
            Error::Syntax
        );
    }
}
