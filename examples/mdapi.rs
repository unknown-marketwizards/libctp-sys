#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use ctp::*;

use std::ffi::{CStr, CString};
use std::os::raw::*;
use std::time::Duration;

use crossbeam::{
    channel::{self, Receiver, Sender},
    select,
};
use log::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    flow_path: String,
    is_udp: bool,
    is_multicast: bool,
    front_addr: Vec<String>,

    #[serde(default)]
    symbols_file: String,
}

pub struct MDApi {
    api: Rust_CThostFtdcMdApi,
    spi: Option<*mut Rust_CThostFtdcMdSpi>,
    rx: Option<Receiver<Event>>,

    pub(crate) config: Config,
}

#[derive(Debug)]
pub enum Event {
    Connected,
    UserLogin,
    Disconnected(i32),

    Unhandled(String),
}

struct Spi {
    tx: Sender<Event>,
}

impl Rust_CThostFtdcMdSpi_Trait for Spi {
    fn on_front_connected(&mut self) {
        debug!("connected.");
        self.tx.send(Event::Connected).unwrap();
    }

    fn on_front_disconnected(&mut self, _nReason: ::std::os::raw::c_int) {
        debug!("front_disconnected");
    }

    fn on_heart_beat_warning(&mut self, _nTimeLapse: ::std::os::raw::c_int) {
        debug!("heart_beating");
    }

    fn on_rsp_user_login(
        &mut self,
        _pRspUserLogin: *mut CThostFtdcRspUserLoginField,
        _pRspInfo: *mut CThostFtdcRspInfoField,
        _nRequestID: ::std::os::raw::c_int,
        _bIsLast: bool,
    ) {
        debug!("user login");
        self.tx.send(Event::UserLogin).unwrap();
    }

    fn on_rsp_error(
        &mut self,
        _pRspInfo: *mut CThostFtdcRspInfoField,
        _nRequestID: ::std::os::raw::c_int,
        _bIsLast: bool,
    ) {
        debug!("rsp_error");
    }

    fn on_rtn_depth_market_data(&mut self, pDepthMarketData: *mut CThostFtdcDepthMarketDataField) {
        if pDepthMarketData.is_null() {
            warn!("got empty data");
        } else {
            let pDepthMarketData = unsafe { &mut *pDepthMarketData };
            debug!("got depth data: {:?}", pDepthMarketData);
        }
    }
}

impl MDApi {
    pub fn get_version() -> String {
        let cs = unsafe { CStr::from_ptr(CThostFtdcMdApi::GetApiVersion()) };
        cs.to_string_lossy().into()
    }

    pub fn new(config: &Config) -> Self {
        let cs = std::ffi::CString::new(config.flow_path.as_bytes()).unwrap();
        let api = unsafe {
            Rust_CThostFtdcMdApi::new(CThostFtdcMdApi::CreateFtdcMdApi(
                cs.as_ptr(),
                config.is_udp,
                config.is_multicast,
            ))
        };
        Self {
            api,
            spi: None,
            config: config.clone(),
            rx: None,
        }
    }

    fn req_init(&mut self) -> Result<(), String> {
        let (tx, rx) = channel::bounded(1024);
        self.register(Spi { tx });
        self.rx = Some(rx);
        debug!("start api...");

        for addr in &self.config.front_addr {
            debug!("front_addr is: {}", addr);
            let cs = CString::new(addr.as_bytes()).unwrap();
            unsafe {
                self.api.RegisterFront(cs.as_ptr() as *mut _);
            }
        }

        unsafe {
            self.api.Init();
        }

        Ok(())
    }

    fn req_user_login(&mut self) -> Result<(), String> {
        // let loginfield : CThostFtdcReqUserLoginField = todo!();
        let mut loginfield = CThostFtdcReqUserLoginField {
            TradingDay: Default::default(),
            BrokerID: Default::default(),
            UserID: Default::default(),
            Password: [0i8; 41],
            UserProductInfo: Default::default(),
            InterfaceProductInfo: Default::default(),
            ProtocolInfo: Default::default(),
            MacAddress: Default::default(),
            OneTimePassword: [0i8; 41],
            ClientIPAddress: [0; 33],
            LoginRemark: [0i8; 36],
            ClientIPPort: Default::default(),
            reserve1: [0; 16],
        };

        unsafe {
            self.api.ReqUserLogin(&mut loginfield, 1);
        }
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.req_init()?;
        assert!(self.rx.is_some(), "channel not started.");

        let rx = self.rx.as_mut().unwrap();

        loop {
            select! {
                recv(rx) -> event => {
                    match event {
                        Ok(Event::Connected) => {
                            break;
                        }
                        _ => {
                            return Err(format!("invalid event: {:?}", event))
                        }
                    }
                },
                default((Duration::from_secs(5))) => {
                    return Err("Timeout try recv `req_init`".into())
                }
            }
        }

        self.req_user_login()?;

        let rx = self.rx.as_mut().unwrap();
        loop {
            select! {
                recv(rx) -> event => {
                    match event {
                        Ok(Event::UserLogin) => {
                            break;
                        }
                        _ => {
                            return Err(format!("invalid event: {:?}", event))
                        }
                    }
                },
                default((Duration::from_secs(5))) => {
                    return Err("Timeout try recv `req_user_login`".into())
                }
            }
        }

        Ok(())
    }

    pub fn subscribe_market_data(&mut self, codes: &[&str], is_unsub: bool) -> Result<(), String> {
        let len = codes.len() as c_int;
        let arr_cstring: Vec<CString> = codes
            .iter()
            .map(|s| CString::new(s.as_bytes()).unwrap())
            .collect();
        let arr_cstr: Vec<*mut c_char> = arr_cstring
            .iter()
            .map(|s| s.as_ptr() as *mut c_char)
            .collect();
        let ptr = arr_cstr.as_ptr() as *mut *mut c_char;
        let rtn = if is_unsub {
            unsafe { self.api.UnSubscribeMarketData(ptr, len) }
        } else {
            unsafe { self.api.SubscribeMarketData(ptr, len) }
        };
        if rtn != 0 {
            return Err(format!(
                "Fail to req `md_api_subscribe_market_data`: {}",
                rtn
            ));
        }

        Ok(())
    }

    fn register<S: Rust_CThostFtdcMdSpi_Trait>(&mut self, spi: S) {
        if let Some(spi) = self.spi.take() {
            debug!("des old registered spi");
            Self::drop_spi(spi);
        }

        let spi: Box<Box<dyn Rust_CThostFtdcMdSpi_Trait>> = Box::new(Box::new(spi));
        let ptr = Box::into_raw(spi) as *mut _ as *mut c_void;

        let spi_stub = unsafe { Rust_CThostFtdcMdSpi::new(ptr) };
        let spi: *mut Rust_CThostFtdcMdSpi = Box::into_raw(Box::new(spi_stub));
        unsafe {
            self.api.RegisterSpi(spi as _);
        }

        self.spi = Some(spi);
    }

    fn drop_spi(spi: *mut Rust_CThostFtdcMdSpi) {
        let mut spi = unsafe { Box::from_raw(spi) };
        unsafe {
            spi.destruct();
        }
    }
}

impl Drop for MDApi {
    fn drop(&mut self) {
        debug!("drop api");
        unsafe {
            self.api.destruct();
        }
        if let Some(spi) = self.spi {
            debug!("drop spi");
            Self::drop_spi(spi);
        }
    }
}

pub fn main() {
    env_logger::init();

    info!("load mdapi: {}", MDApi::get_version());

    let mut mdapi = MDApi::new(&Config {
        flow_path: "".into(),

        // simnow - full
        front_addr: vec![
            "tcp://180.168.146.187:10211".into(),
            "tcp://180.168.146.187:10212".into(),
            "tcp://218.202.237.33:10213".into(),
        ],

        ..Default::default()
    });

    mdapi.start().unwrap();
    mdapi.subscribe_market_data(&["rb2201"], false).unwrap();
    let mut count = 0;

    eprintln!("mk api success");
    if let Some(ref mut rx) = mdapi.rx {
        while let Ok(event) = rx.recv() {
            count += 1;
            debug!("Got event: {:?}", event);
            if count >= 5 {
                break;
            }
        }
    }
}
