use std::time::Duration;

use bird_core::{Certificate, Hostname};
use bird_proxy::Challenges;
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, NewOrder, OrderStatus, RetryPolicy,
};

use super::expiry::not_after;
use crate::{Error, Result};

const POLL: RetryPolicy = RetryPolicy::new().timeout(Duration::from_secs(90));

pub(super) async fn issue(
    account: &Account,
    challenges: &Challenges,
    hostname: &Hostname,
) -> Result<Certificate> {
    let mut tokens = Vec::new();
    let result = order(account, challenges, hostname, &mut tokens).await;
    for token in &tokens {
        challenges.remove(token);
    }
    let (chain_pem, key_pem) = result?;
    Ok(Certificate {
        hostname: hostname.clone(),
        not_after: not_after(&chain_pem)?,
        chain_pem,
        key_pem,
    })
}

async fn order(
    account: &Account,
    challenges: &Challenges,
    hostname: &Hostname,
    tokens: &mut Vec<String>,
) -> Result<(String, String)> {
    let identifiers = [Identifier::Dns(hostname.to_string())];
    let mut order = account.new_order(&NewOrder::new(&identifiers)).await?;

    {
        let mut authorizations = order.authorizations();
        while let Some(authorization) = authorizations.next().await {
            let mut authorization = authorization?;
            match authorization.status {
                AuthorizationStatus::Pending => {}
                AuthorizationStatus::Valid => continue,
                status => {
                    return Err(Error::Certificate(format!("authorization is {status:?}")));
                }
            }
            let mut challenge = authorization
                .challenge(ChallengeType::Http01)
                .ok_or_else(|| Error::Certificate("ca offered no http-01 challenge".to_owned()))?;
            let token = challenge.token.clone();
            challenges.insert(
                token.clone(),
                challenge.key_authorization().as_str().to_owned(),
            );
            tokens.push(token);
            challenge.set_ready().await?;
        }
    }

    let status = order.poll_ready(&POLL).await?;
    if status != OrderStatus::Ready {
        return Err(Error::Certificate(format!("order ended as {status:?}")));
    }
    let key_pem = order.finalize().await?;
    let chain_pem = order.poll_certificate(&POLL).await?;
    Ok((chain_pem, key_pem))
}
