#![allow(non_snake_case)]

mod proxy;

use multiversx_sc_snippets::{imports::*};

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
    let cmd = args.next().expect("at least one argument required");
    let mut interact = ContractInteract::new().await;
    match cmd.as_str() {
        "deploy" => interact.deploy().await,
        "setExactValueFee" => interact.set_exact_value_fee().await,
        "setPercentageFee" => interact.set_percentage_fee().await,
        "claimFees" => interact.claim_fees().await,
        "transfer" => interact.transfer().await,
        "getTokenFee" => interact.token_fee().await,
        "getPaidFees" => interact.paid_fees().await,
        _ => panic!("unknown command: {}", &cmd),
    }
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
            .gas(NumExpr("30,000,000"))
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

    async fn set_exact_value_fee(&mut self) {
        let fee_token = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let fee_amount = BigUint::<StaticApi>::from(0u128);
        let token = TokenIdentifier::from_esdt_bytes(&b""[..]);

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

    async fn set_exact_value_fee_with_params(&mut self) {
       
        
        let fee_token = TokenIdentifier::from_esdt_bytes(&b"TESTDOI-fe9ac5-01"[..]);
        let fee_amount = BigUint::<StaticApi>::from(1u128);
        let token = TokenIdentifier::from_esdt_bytes(&b"TESTDOI-fe9ac5-01"[..]);



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



    async fn set_percentage_fee(&mut self) {
        let fee = 0u32;
        let token = TokenIdentifier::from_esdt_bytes(&b""[..]);

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
            .returns(ExpectError(4u64, "There is nothing to claim"))
           // .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }

    // Owner-ul seteaza in storage ExactValueFee: 
    // daca user-ul face transfer cu fee-ul corespunzator (acelasi token + acelasi amount),
    // identic cu cel din storage, se face transferul si se salveaza fee-ul in paid_fees.


    async fn transfer_with_fee(&mut self) {

        //let token_id = String::from("TEST-36db68-01");
        let token_nonce = 0u64;
        let token_amount = BigUint::<StaticApi>::from(1u128);
        
        //let address = bech32::decode("");
        
        // doua tranzactii la rand: token-ul si fee-ul
        let mut transactions = ManagedVec::new();
        transactions.push((EsdtTokenPayment::new(TokenIdentifier::from_esdt_bytes(&b"TESTDOI-fe9ac5-01"[..]), token_nonce, token_amount.clone())));
        transactions.push((EsdtTokenPayment::new(TokenIdentifier::from_esdt_bytes(&b"TESTDOI-fe9ac5-01"[..]), token_nonce, token_amount.clone())));

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



    async fn transfer_without_fee(&mut self) {

        let token_nonce = 0u64;
        let token_amount = BigUint::<StaticApi>::from(1u128);
        
      
        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
         
            .typed(proxy::EsdtTransferWithFeeProxy)
            .transfer(&self.wallet_address)
            .payment(((TokenIdentifier::from_esdt_bytes(&b"TESTDOI-fe9ac5-01"[..]), token_nonce, token_amount)))
            .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }



    async fn token_fee(&mut self) {
        let token = TokenIdentifier::from_esdt_bytes(&b"TESTDOI-fe9ac5-01"[..]);

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


   

    async fn transfer(&mut self) {
        let token_id = String::new();
        let token_nonce = 0u64;
        let token_amount = BigUint::<StaticApi>::from(0u128);

        let address = bech32::decode("");

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(NumExpr("30,000,000"))
                // aici cred ca ar trebui adaugat transferul: .esdt .esdt
            //.esdt(payment)
            .typed(proxy::EsdtTransferWithFeeProxy)
            .transfer(address)
            .payment((TokenIdentifier::from(token_id.as_str()), token_nonce, token_amount))
            .returns(ReturnsResultUnmanaged)
            .prepare_async()
            .run()
            .await;

        println!("Result: {response:?}");
    }

}


#[tokio::test]
async fn test_set_exact_value(){

    let mut interactor = ContractInteract::new().await;

    interactor.deploy().await;

    interactor.paid_fees().await;
    
    interactor.set_exact_value_fee_with_params().await;

    interactor.token_fee().await;

}


#[tokio::test]
async fn test_a_happy_transfer(){

    let mut interactor = ContractInteract::new().await;

    interactor.deploy().await;

    interactor.paid_fees().await;
    
    interactor.set_exact_value_fee_with_params().await;

    interactor.transfer_with_fee().await;

    interactor.paid_fees().await;

}




#[tokio::test]
async fn test_a_failed_transfer(){

    let mut interactor = ContractInteract::new().await;

    interactor.deploy().await;

    interactor.paid_fees().await;
    
    interactor.set_exact_value_fee_with_params().await;

    interactor.transfer_without_fee().await;

    interactor.paid_fees().await;

}



