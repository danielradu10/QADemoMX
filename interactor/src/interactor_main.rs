#![allow(non_snake_case)]

mod proxy;

use multiversx_sc_snippets::imports::*;

use crate::sdk::wallet::Wallet;
use multiversx_sc_snippets::sdk;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};


const GATEWAY: &str = sdk::gateway::DEVNET_GATEWAY;
//blockchain::DEVNET_GATEWAY;
const STATE_FILE: &str = "state.toml";


#[tokio::main]
async fn main() {
    env_logger::init();

    let mut args = std::env::args();
    let _ = args.next();
    //let cmd = args.next().expect("at least one argument required");
    //let mut interact = ContractInteract::new().await;
    // match cmd.as_str() {
    //     "deploy" => interact.deploy().await,
    //     "setExactValueFee" => interact.set_exact_value_fee().await,
    //     "setPercentageFee" => interact.set_percentage_fee().await,
    //     "claimFees" => interact.claim_fees().await,
    //     "transfer" => interact.transfer().await,
    //     "getTokenFee" => interact.token_fee().await,
    //     "getPaidFees" => interact.paid_fees().await,
    //     _ => panic!("unknown command: {}", &cmd),
    // }
}


#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    contract_address: Option<Bech32Address>
}

impl State {
        // Deserializes state from file
        pub fn load_state() -> Self {
            if Path::new(STATE_FILE).exists() {
                let mut file = std::fs::File::open(STATE_FILE).unwrap();
                let mut content = String::new();
                file.read_to_string(&mut content).unwrap();
                toml::from_str(&content).unwrap()
            } else {
                Self::default()
            }
        }
    
        /// Sets the contract address
        pub fn set_address(&mut self, address: Bech32Address) {
            self.contract_address = Some(address);
        }
    
        /// Returns the contract address
        pub fn current_address(&self) -> &Bech32Address {
            self.contract_address
                .as_ref()
                .expect("no known contract, deploy first")
        }
    }
    
    impl Drop for State {
        // Serializes state to file
        fn drop(&mut self) {
            let mut file = std::fs::File::create(STATE_FILE).unwrap();
            file.write_all(toml::to_string(self).unwrap().as_bytes())
                .unwrap();
        }
    }

struct ContractInteract {
    interactor: Interactor,
    wallet_address: Address,
    contract_code: BytesValue,
    state: State
}

impl ContractInteract {
    async fn new() -> Self {
        let mut interactor = Interactor::new(GATEWAY).await;
        let wallet_address = interactor.register_wallet(Wallet::from_pem_file("walletNew.pem").unwrap());
    
        let contract_code = BytesValue::interpret_from(
            "mxsc:../output/esdt-transfer-with-fee.mxsc.json",
            &InterpreterContext::default(),
        );

        ContractInteract {
            interactor,
            wallet_address,
            contract_code,
            state: State::load_state()
        }
    }

    async fn deploy(&mut self) {
        let new_address = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .gas(NumExpr("35,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .init()
            .code_metadata(CodeMetadata::PAYABLE) // adaugat, contractul sa fie payable
            .code(&self.contract_code)
            .returns(ReturnsNewAddress)
            .prepare_async()
            .run()
            .await;
        let new_address_bech32 = bech32::encode(&new_address);
        self.state
            .set_address(Bech32Address::from_bech32_string(new_address_bech32.clone()));

        println!("new address: {new_address_bech32}");
    }

    async fn set_exact_value_fee(&mut self, fee_token: TokenIdentifier<StaticApi>, fee_amount: BigUint<StaticApi>, token: TokenIdentifier<StaticApi>) {

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .set_exact_value_fee(fee_token, fee_amount, token)
            .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn set_percentage_fee(&mut self, fee: u32, token: TokenIdentifier<StaticApi>) {

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .set_percentage_fee(fee, token)
            .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn claim_fees(&mut self) {
        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .claim_fees()
            .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn claim_fees_error(&mut self, expected_result: ExpectError<'_>) {
        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .claim_fees()
            .returns(expected_result)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }

    // Owner-ul seteaza in storage ExactValueFee: 
    // daca user-ul face transfer cu fee-ul corespunzator (acelasi token + acelasi amount),
    // identic cu cel din storage, se face transferul si se salveaza fee-ul in paid_fees.
    // TO DO: receive the parameters (the tokens)
    async fn transfer_with_fee(
        &mut self,
        token_identifier: TokenIdentifier<StaticApi>,
        token_amount: BigUint<StaticApi>,
        fee_token_identifier: TokenIdentifier<StaticApi>,
        fee_amount: BigUint<StaticApi>
    ) {
        // doua tranzactii la rand: token-ul si fee-ul
        let token_nonce = 0u64;
        let mut transactions = ManagedVec::new();
        transactions.push((EsdtTokenPayment::new(token_identifier, token_nonce, token_amount)));
        transactions.push((EsdtTokenPayment::new(fee_token_identifier, token_nonce, fee_amount)));

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .transfer(&self.wallet_address)
            .payment(transactions)
            .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }


    async fn transfer_with_miss_fee(    //inainte de refactor pentru ca da failed
        &mut self,
        expected_result: ExpectError<'_>
    ) {
        let token_nonce = 0u64; 
        let fee_token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
        let fee_amount = BigUint::<StaticApi>::from(9u128);
        let token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
        let token_amount = BigUint::<StaticApi>::from(13u64);       

        // doua tranzactii la rand: token-ul si fee-ul
        let mut transactions = ManagedVec::new();
        transactions.push((EsdtTokenPayment::new(token, token_nonce, token_amount)));
        transactions.push((EsdtTokenPayment::new(fee_token, token_nonce, fee_amount)));

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .transfer(&self.wallet_address)
            .payment(transactions)
            .returns(expected_result)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }



    // this is a transfer without any fee. It should return "Fee payment missing"
    async fn transfer_without_fee(
        &mut self, 
        expected_result: ExpectError<'_>,
        token: TokenIdentifier<StaticApi>, 
        token_amount: BigUint<StaticApi>
    ) { 

        let token_nonce = 0u64;
        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .transfer(&self.wallet_address)
            .payment(((token, token_nonce, token_amount)))
            .returns(expected_result)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn transfer_without_proper_fee(
            &mut self, expected_result: ExpectError<'_>,
            token_identifier: TokenIdentifier<StaticApi>,
            token_amount: BigUint<StaticApi>,
            fee_token_identifier: TokenIdentifier<StaticApi>,
            fee_amount: BigUint<StaticApi>
        ) {
        let token_nonce = 0u64;   
    
        let mut transactions = ManagedVec::new();
        transactions.push(EsdtTokenPayment::new(token_identifier, token_nonce, token_amount));
        transactions.push(EsdtTokenPayment::new(fee_token_identifier, token_nonce, fee_amount));

    
        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .transfer(&self.wallet_address)
            .payment(transactions)
            .returns(expected_result)
            .prepare_async()
            .run()
            .await;
    
        println!("Result: {response:?}");
    }

    async fn token_fee(&mut self, token: TokenIdentifier<StaticApi>) {

        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::EsdtTransferWithFeeProxy)
            .token_fee(token)
            .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn paid_fees(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::EsdtTransferWithFeeProxy)
            .paid_fees()
            .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn transfer(&mut self, token_id: String, token_nonce: u64, token_amount: BigUint<StaticApi>) {

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .transfer(&self.wallet_address)
            .payment((TokenIdentifier::from(token_id.as_str()), token_nonce, token_amount))
            .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }


    // this is a transfer with egld. It should return ""EGLD transfers not allowed""
    async fn transfer_with_egld(&mut self, token_amount: BigUint<StaticApi>) {
  
        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
            .typed(proxy::EsdtTransferWithFeeProxy)
            .transfer(&self.wallet_address)
            .egld(token_amount)
            .returns(ExpectError(4, "EGLD transfers not allowed"))
            .prepare_async()
            .run() 
            .await;

        println!("Result: {response:?}");
    }

}


// // this test should set the expected fee in storage
 #[tokio::test]
async fn test_set_exact_value(){

    let mut interactor = ContractInteract::new().await;

    interactor.deploy().await;

    let fee_token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(1u128);
    let token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);

    interactor.paid_fees().await;
    
    interactor.set_exact_value_fee(fee_token,fee_amount, token.clone()).await;

    interactor.token_fee(token.clone()).await;

}


// // this test should show elements in the paid_fees vector
 #[tokio::test]
async fn test_a_happy_transfer(){

    let mut interactor = ContractInteract::new().await;

    interactor.deploy().await;

    let fee_token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(1u128);
    let token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);

    interactor.set_exact_value_fee(token,fee_amount,fee_token).await;

    interactor.paid_fees().await;

    let token_amount = BigUint::<StaticApi>::from(1000u64);
    let token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(2u128);

    interactor.transfer_with_fee(token_identifier,token_amount,fee_token_identifier,fee_amount).await;

    interactor.paid_fees().await;

}

// // transfer, show paid fees, claim, show paid fees again (this time empty)
 #[tokio::test]
async fn test_a_happy_transfer_with_claim(){

    let mut interactor = ContractInteract::new().await;

    interactor.deploy().await;

    let fee_token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(1u128);
    let token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);

    interactor.paid_fees().await;
    
    interactor.set_exact_value_fee(token,fee_amount,fee_token).await;

    let token_amount = BigUint::<StaticApi>::from(1000u64);
    let token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(2u128);

    interactor.transfer_with_fee(token_identifier,token_amount,fee_token_identifier,fee_amount).await;

    interactor.paid_fees().await;

    interactor.claim_fees().await;

    interactor.paid_fees().await;

}


// this test should return "Fee payment missing"
#[tokio::test]
async fn test_a_failed_transfer(){

    let mut interactor = ContractInteract::new().await;
    let fee_token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(1u128);
    let token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let token_amount = BigUint::<StaticApi>::from(1000u64);

    interactor.deploy().await;

    interactor.paid_fees().await;
    
    interactor.set_exact_value_fee(token,fee_amount,fee_token).await;

    let token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);

    interactor.transfer_without_fee(ExpectError(4, "Fee payment missing"), token_identifier, token_amount ).await;
}



// this test should return "EGLD transfers not allowed"
#[tokio::test]
async fn test_simple_transfer_with_egld(){

    let mut interactor = ContractInteract::new().await;
    let fee_token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(1u128);
    let token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let token_amount = BigUint::<StaticApi>::from(1000u64);

    interactor.deploy().await;

    interactor.paid_fees().await;
    
    interactor.set_exact_value_fee(token, fee_amount, fee_token).await;

    interactor.transfer_with_egld(token_amount).await;
}

// this test should return "There is nothing to claim"
#[tokio::test]
async fn test_claim_with_no_fees() {        
    let mut interactor = ContractInteract::new().await;

    interactor.deploy().await;

    interactor.claim_fees_error(ExpectError(4, "There is nothing to claim")).await;

}

// this test should return "Wrong fee token"
#[tokio::test]
async fn test_transfer_with_wrong_fee_token() {
    let mut interactor = ContractInteract::new().await;
    let fee_token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(1u128);
    let token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);

    interactor.deploy().await;

    interactor.set_exact_value_fee(token, fee_amount,fee_token).await;

    let token_amount = BigUint::<StaticApi>::from(1000u64);
    let token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_token_identifier = TokenIdentifier::from_esdt_bytes(&b"XMAS-43a751"[..]);
    let fee_amount = BigUint::<StaticApi>::from(2u128);

    interactor.transfer_without_proper_fee(ExpectError(4, "Wrong fee token"),token_identifier,token_amount,fee_token_identifier,fee_amount).await;

}

// this test should make a transaction with a percentage fee
#[tokio::test]
async fn test_transfer_percentage_fee(){

    let mut interactor = ContractInteract::new().await;
    let fee_token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_percentage = 10u32;

    interactor.deploy().await;

    interactor.paid_fees().await;
    
    interactor.set_percentage_fee(fee_percentage, fee_token).await;

    let token_amount = BigUint::<StaticApi>::from(1000u64);
    let token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(2u128);

    interactor.transfer_with_fee(token_identifier,token_amount,fee_token_identifier,fee_amount).await;

    interactor.paid_fees().await;

    interactor.claim_fees().await;

    interactor.paid_fees().await;

}


#[tokio::test]
async fn test_missmatching_payment_fee(){       //da failed 

    let mut interactor = ContractInteract::new().await;
     let fee_token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
     let fee_amount = BigUint::<StaticApi>::from(10u128);
     let token = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    // let token_amount = BigUint::<StaticApi>::from(100u64);

    interactor.deploy().await;
    interactor.set_exact_value_fee(token,fee_amount,fee_token).await;
    interactor.transfer_with_miss_fee(ExpectError(4, "Mismatching payment for covering fees")).await;
}

#[tokio::test]
async fn test_fee_not_set(){

    let mut interactor = ContractInteract::new().await;
    interactor.deploy().await;
    let token_amount = BigUint::<StaticApi>::from(1000u64);
    let token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_token_identifier = TokenIdentifier::from_esdt_bytes(&b"TOKENTEST-b0b548"[..]);
    let fee_amount = BigUint::<StaticApi>::from(2u128);

    interactor.transfer_with_fee(token_identifier,token_amount,fee_token_identifier,fee_amount).await;
}


