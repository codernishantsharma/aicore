use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_512};
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequirements {
    pub persona: Option<String>,
    pub arkose: Option<serde_json::Value>,
    pub turnstile: Option<serde_json::Value>,
    pub proofofwork: Option<ProofOfWorkConfig>,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofOfWorkConfig {
    pub required: bool,
    pub seed: String,
    pub difficulty: Option<String>,
}

pub struct PowSolver;

impl PowSolver {
    pub fn solve(seed: &str, difficulty: &str, user_agent: &str) -> Option<String> {
        let diff_len = difficulty.len();
        debug!(seed, difficulty, "Solving ChatGPT Proof-of-Work...");

        for nonce in 0..500_000 {
            let candidate = format!("{}:{}:{}", seed, nonce, user_agent);
            let mut hasher = Sha3_512::new();
            hasher.update(candidate.as_bytes());
            let result = hasher.finalize();
            let hex_result = hex::encode(result);

            if difficulty.is_empty() || hex_result.starts_with(difficulty) {
                let proof_token = format!("gAAAAAB_{}_{}_{}", seed, nonce, &hex_result[..diff_len.min(16)]);
                return Some(proof_token);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pow_solver() {
        let seed = "test_seed_123";
        let difficulty = "0"; // matches any hash starting with '0'
        let user_agent = "Mozilla/5.0";

        let solution = PowSolver::solve(seed, difficulty, user_agent);
        assert!(solution.is_some());
        let proof = solution.unwrap();
        assert!(proof.contains("gAAAAAB_test_seed_123_"));
    }
}
