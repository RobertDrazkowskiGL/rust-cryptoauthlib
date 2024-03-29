use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::{From, TryFrom};
use std::ptr;
use std::sync::Mutex;

#[cfg(test)]
use cryptoauthlib_sys::atca_aes_cbc_ctx_t;
#[cfg(test)]
use cryptoauthlib_sys::atca_aes_ctr_ctx_t;

use super::{
    AeadAlgorithm, AeadParam, AtcaAesCcmCtx, AtcaDeviceType, AtcaIfaceCfg, AtcaIfaceCfgPtrWrapper,
    AtcaIfaceType, AtcaSlot, AtcaSlotCapacity, AtcaStatus, AteccDeviceTrait, ChipOptions,
    CipherAlgorithm, CipherOperation, CipherParam, EccKeyAttr, FeedbackMode, InfoCmdType, KeyType,
    NonceTarget, OutputProtectionState, ReadKey, SignMode, SlotConfig, VerifyMode, WriteConfig,
};
use super::{
    ATCA_AES_DATA_SIZE, ATCA_AES_GCM_IV_STD_LENGTH, ATCA_AES_KEY_SIZE,
    ATCA_ATECC_CONFIG_BUFFER_SIZE, ATCA_ATECC_MIN_SLOT_IDX_FOR_PUB_KEY, ATCA_ATECC_PRIV_KEY_SIZE,
    ATCA_ATECC_PUB_KEY_SIZE, ATCA_ATECC_SLOTS_COUNT, ATCA_ATECC_TEMPKEY_KEYID,
    ATCA_ATSHA_CONFIG_BUFFER_SIZE, ATCA_BLOCK_SIZE, ATCA_KEY_SIZE, ATCA_LOCK_ZONE_CONFIG,
    ATCA_LOCK_ZONE_DATA, ATCA_NONCE_NUMIN_SIZE, ATCA_NONCE_SIZE, ATCA_RANDOM_BUFFER_SIZE,
    ATCA_SERIAL_NUM_SIZE, ATCA_SHA2_256_DIGEST_SIZE, ATCA_SIG_SIZE, ATCA_ZONE_CONFIG,
    ATCA_ZONE_DATA,
};

mod aes_ccm;
mod aes_cipher;
mod aes_gcm;
mod c2rust;
mod rust2c;

struct AteccResourceManager {
    ref_counter: u8,
}

lazy_static! {
    static ref ATECC_RESOURCE_MANAGER: Mutex<AteccResourceManager> =
        Mutex::new(AteccResourceManager { ref_counter: 0 });
}

impl AteccResourceManager {
    // Aquire an acceptance to create an ATECC instance
    fn acquire(&mut self) -> bool {
        if self.ref_counter == 0 {
            self.ref_counter = 1;
            true
        } else {
            false
        }
    }

    // Release a reservation of an ATECC instance
    fn release(&mut self) -> bool {
        if self.ref_counter == 1 {
            self.ref_counter = 0;
            true
        } else {
            false
        }
    }
}

/// An ATECC cryptochip context holder.
#[derive(Debug)]
pub struct AteccDevice {
    /// Interface configuration to be stored on a heap to avoid side effects of
    /// Rust and C interoperability
    iface_cfg_ptr: AtcaIfaceCfgPtrWrapper,
    /// A mutex to ensure a mutual access from different threads to an ATECC instance
    api_mutex: Mutex<()>,
    serial_number: [u8; ATCA_SERIAL_NUM_SIZE],
    config_zone_locked: bool,
    data_zone_locked: bool,
    chip_options: ChipOptions,
    access_keys: Mutex<RefCell<HashMap<u8, [u8; ATCA_KEY_SIZE]>>>,
    slots: Vec<AtcaSlot>,
}

impl Default for AteccDevice {
    fn default() -> AteccDevice {
        AteccDevice {
            iface_cfg_ptr: AtcaIfaceCfgPtrWrapper {
                ptr: std::ptr::null_mut(),
            },
            api_mutex: Mutex::new(()),
            serial_number: [0; ATCA_SERIAL_NUM_SIZE],
            config_zone_locked: false,
            data_zone_locked: false,
            chip_options: Default::default(),
            access_keys: Mutex::new(RefCell::new(HashMap::new())),
            slots: Vec::new(),
        }
    }
}

impl AteccDeviceTrait for AteccDevice {
    /// Request ATECC to generate a vector of random bytes
    /// Trait implementation
    fn random(&self, rand_out: &mut Vec<u8>) -> AtcaStatus {
        self.random(rand_out)
    } // AteccDevice::random()

    /// Request ATECC to compute a message hash (SHA256)
    /// Trait implementation
    fn sha(&self, message: Vec<u8>, digest: &mut Vec<u8>) -> AtcaStatus {
        self.sha(message, digest)
    } // AteccDevice::sha()

    /// Execute a Nonce command in pass-through mode to load one of the
    /// device's internal buffers with a fixed value.
    /// For the ATECC608A, available targets are TempKey (32 or 64 bytes), Message
    /// Digest Buffer (32 or 64 bytes), or the Alternate Key Buffer (32 bytes). For
    /// all other devices, only TempKey (32 bytes) is available.
    /// Trait implementation
    fn nonce(&self, target: NonceTarget, data: &[u8]) -> AtcaStatus {
        self.nonce(target, data)
    } // AteccDevice::nonce()

    /// Execute a Nonce command to generate a random nonce combining a host
    /// nonce and a device random number.
    /// Trait implementation
    fn nonce_rand(&self, host_nonce: &[u8], rand_out: &mut Vec<u8>) -> AtcaStatus {
        self.nonce_rand(host_nonce, rand_out)
    } // AteccDevice::nonce_rand()

    /// Request ATECC to generate a cryptographic key
    /// Trait implementation
    fn gen_key(&self, key_type: KeyType, slot_id: u8) -> AtcaStatus {
        self.gen_key(key_type, slot_id)
    } // AteccDevice::gen_key()

    /// Request ATECC to import a cryptographic key
    /// Trait implementation
    fn import_key(&self, key_type: KeyType, key_data: &[u8], slot_id: u8) -> AtcaStatus {
        self.import_key(key_type, key_data, slot_id)
    } // AteccDevice::import_key()

    /// Request ATECC to export a cryptographic key
    /// Trait implementation
    fn export_key(&self, key_type: KeyType, key_data: &mut Vec<u8>, slot_id: u8) -> AtcaStatus {
        self.export_key(key_type, key_data, slot_id)
    } // AteccDevice::export_key()

    /// Depending on the socket configuration, this function calculates
    /// public key based on an existing private key in the socket
    /// or exports the public key directly
    /// Trait implementation
    fn get_public_key(&self, slot_id: u8, public_key: &mut Vec<u8>) -> AtcaStatus {
        self.get_public_key(slot_id, public_key)
    } // AteccDevice::get_public_key()

    /// Request ATECC to generate an ECDSA signature
    /// Trait implementation
    fn sign_hash(&self, mode: SignMode, slot_id: u8, signature: &mut Vec<u8>) -> AtcaStatus {
        self.sign_hash(mode, slot_id, signature)
    } // AteccDevice::sign_hash()

    /// Request ATECC to verify ECDSA signature
    /// Trait implementation
    fn verify_hash(
        &self,
        mode: VerifyMode,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AtcaStatus> {
        self.verify_hash(mode, hash, signature)
    } // AteccDevice::verify_hash()

    /// Data encryption function in AES unauthenticated cipher alhorithms modes
    /// Trait implementation
    fn cipher_encrypt(
        &self,
        algorithm: CipherAlgorithm,
        slot_id: u8,
        data: &mut Vec<u8>,
    ) -> AtcaStatus {
        self.cipher_encrypt(algorithm, slot_id, data)
    } // AteccDevice::cipher_encrypt()

    /// Data decryption function in AES unauthenticated cipher alhorithms modes
    /// Trait implementation
    fn cipher_decrypt(
        &self,
        algorithm: CipherAlgorithm,
        slot_id: u8,
        data: &mut Vec<u8>,
    ) -> AtcaStatus {
        self.cipher_decrypt(algorithm, slot_id, data)
    } // AteccDevice::cipher_decrypt()

    /// Data encryption function in AES AEAD (authenticated encryption with associated data) modes
    /// Trait implementation
    fn aead_encrypt(
        &self,
        algorithm: AeadAlgorithm,
        slot_id: u8,
        data: &mut Vec<u8>,
    ) -> Result<Vec<u8>, AtcaStatus> {
        self.aead_encrypt(algorithm, slot_id, data)
    } // AteccDevice::aead_encrypt()

    /// Data decryption function in AES AEAD (authenticated encryption with associated data) modes
    /// Trait implementation
    fn aead_decrypt(
        &self,
        algorithm: AeadAlgorithm,
        slot_id: u8,
        data: &mut Vec<u8>,
    ) -> Result<bool, AtcaStatus> {
        self.aead_decrypt(algorithm, slot_id, data)
    } // AteccDevice::aead_decrypt()

    /// Request ATECC to return own device type
    /// Trait implementation
    fn get_device_type(&self) -> AtcaDeviceType {
        self.get_device_type()
    } // AteccDevice::get_device_type()

    /// Request ATECC to check if its configuration is locked.
    /// If true, a chip can be used for cryptographic operations
    /// Trait implementation
    fn is_configuration_locked(&self) -> bool {
        self.config_zone_locked
    } // AteccDevice::is_configuration_locked()

    /// Request ATECC to check if its Data Zone is locked.
    /// If true, a chip can be used for cryptographic operations
    /// Trait implementation
    fn is_data_zone_locked(&self) -> bool {
        self.data_zone_locked
    } // AteccDevice::is_data_zone_locked()

    /// Returns a structure containing configuration data read from ATECC
    /// during initialization of the AteccDevice object.
    /// Trait implementation
    fn get_config(&self, atca_slots: &mut Vec<AtcaSlot>) -> AtcaStatus {
        self.get_config(atca_slots)
    } // AteccDevice::get_config()

    /// Command accesses some static or dynamic information from the ATECC chip
    /// Trait implementation
    fn info_cmd(&self, command: InfoCmdType) -> Result<Vec<u8>, AtcaStatus> {
        self.info_cmd(command)
    } // AteccDevice::info_cmd()

    /// A function that adds an access key for securely reading or writing data
    /// that is located in a specific slot on the ATECCx08 chip.
    /// Data is not written to the ATECCx08 chip, but to the AteccDevice structure.
    /// Trait implementation
    fn add_access_key(&self, slot_id: u8, access_key: &[u8]) -> AtcaStatus {
        self.add_access_key(slot_id, access_key)
    } // AteccDevice::add_access_key()

    /// A function that deletes all access keys for secure read or write operations
    /// performed by the ATECCx08 chip
    /// Trait implementation
    fn flush_access_keys(&self) -> AtcaStatus {
        self.flush_access_keys()
    } // AteccDevice::flush_access_keys()

    /// Get serial number of the ATECC device
    /// Trait implementation
    fn get_serial_number(&self) -> [u8; ATCA_SERIAL_NUM_SIZE] {
        self.serial_number
    } // AteccDevice::get_serial_number()

    /// Checks if the chip supports AES encryption
    /// (only relevant for the ATECC608x chip)
    /// Trait implementation
    fn is_aes_enabled(&self) -> bool {
        self.chip_options.aes_enabled
    } // AteccDevice::is_aes_enabled()

    /// Checks if the chip supports AES for KDF operations
    /// (only relevant for the ATECC608x chip)
    /// Trait implementation
    fn is_kdf_aes_enabled(&self) -> bool {
        self.chip_options.kdf_aes_enabled
    } // AteccDevice::is_kdf_aes_enabled()

    /// Checks whether transmission between chip and host is to be encrypted
    /// (IO encryption is only possible for ATECC608x chip)
    /// Trait implementation
    fn is_io_protection_key_enabled(&self) -> bool {
        self.chip_options.io_key_enabled
    } // AteccDevice::is_io_protection_key_enabled()

    ///
    /// (only relevant for the ATECC608x chip)
    /// Trait implementation
    fn get_ecdh_output_protection_state(&self) -> OutputProtectionState {
        self.chip_options.ecdh_output_protection
    } // AteccDevice::get_ecdh_output_protection_state()

    ///
    /// (only relevant for the ATECC608x chip)
    /// Trait implementation
    fn get_kdf_output_protection_state(&self) -> OutputProtectionState {
        self.chip_options.kdf_output_protection
    } // AteccDevice::get_kdf_output_protection_state()

    /// ATECC device instance destructor
    /// Trait implementation
    fn release(&self) -> AtcaStatus {
        self.release()
    } // AteccDevice::release()

    //--------------------------------------------------
    //
    // Functions available only during testing
    //
    //--------------------------------------------------

    /// A generic function that reads data from the chip
    /// Trait implementation
    #[cfg(test)]
    fn read_zone(
        &self,
        zone: u8,
        slot: u16,
        block: u8,
        offset: u8,
        data: &mut Vec<u8>,
        len: u8,
    ) -> AtcaStatus {
        self.read_zone(zone, slot, block, offset, data, len)
    } // AteccDevice::read_zone()
    /// Request ATECC to read and return own configuration zone.
    /// Note: this function returns raw data, function get_config(..) implements a more
    /// structured return.
    /// Trait implementation
    #[cfg(test)]
    fn read_config_zone(&self, config_data: &mut Vec<u8>) -> AtcaStatus {
        self.read_config_zone(config_data)
    } // AteccDevice::read_config_zone()

    /// Compare internal config zone contents vs. config_data.
    /// Diagnostic function.
    /// Trait implementation
    #[cfg(test)]
    fn cmp_config_zone(&self, config_data: &mut [u8]) -> Result<bool, AtcaStatus> {
        self.cmp_config_zone(config_data)
    } // AteccDevice::cmp_config_zone()
    /// A function that takes an encryption key for securely reading or writing data
    /// that is located in a specific slot on an ATECCx08 chip.
    /// Data is not taken directly from the ATECCx08 chip, but from the AteccDevice structure
    /// Trait implementation
    #[cfg(test)]
    fn get_access_key(&self, slot_id: u8, key: &mut Vec<u8>) -> AtcaStatus {
        self.get_access_key(slot_id, key)
    } // AteccDevice::get_access_key()
    /// Perform an AES-128 encrypt operation with a key in the device
    /// Trait implementation
    #[cfg(test)]
    fn aes_encrypt_block(
        &self,
        key_id: u16,
        key_block: u8,
        input: &[u8],
    ) -> Result<[u8; ATCA_AES_DATA_SIZE], AtcaStatus> {
        self.aes_encrypt_block(key_id, key_block, input)
    }
    /// Perform an AES-128 decrypt operation with a key in the device
    /// Trait implementation
    #[cfg(test)]
    fn aes_decrypt_block(
        &self,
        key_id: u16,
        key_block: u8,
        input: &[u8],
    ) -> Result<[u8; ATCA_AES_DATA_SIZE], AtcaStatus> {
        self.aes_decrypt_block(key_id, key_block, input)
    }
    /// Initialize context for AES CTR operation with an existing IV
    /// Trait implementation
    #[cfg(test)]
    fn aes_ctr_init(
        &self,
        slot_id: u8,
        counter_size: u8,
        iv: &[u8],
    ) -> Result<atca_aes_ctr_ctx_t, AtcaStatus> {
        self.aes_ctr_init(slot_id, counter_size, iv)
    }
    /// Increments AES CTR counter value
    /// Trait implementation
    #[cfg(test)]
    fn aes_ctr_increment(&self, ctx: atca_aes_ctr_ctx_t) -> Result<atca_aes_ctr_ctx_t, AtcaStatus> {
        self.aes_ctr_increment(ctx)
    }
    /// Initialize context for AES CBC operation.
    /// Trait implementation
    #[cfg(test)]
    fn aes_cbc_init(&self, slot_id: u8, iv: &[u8]) -> Result<atca_aes_cbc_ctx_t, AtcaStatus> {
        self.aes_cbc_init(slot_id, iv)
    }
}

/// Implementation of CryptoAuth Library API Rust wrapper calls
impl AteccDevice {
    /// ATECC device instance constructor
    pub fn new(r_iface_cfg: AtcaIfaceCfg) -> Result<AteccDevice, String> {
        if !ATECC_RESOURCE_MANAGER.lock().unwrap().acquire() {
            return Err(AtcaStatus::AtcaAllocFailure.to_string());
        }
        let iface_cfg = Box::new(
            match cryptoauthlib_sys::ATCAIfaceCfg::try_from(r_iface_cfg) {
                Ok(x) => x,
                Err(()) => {
                    ATECC_RESOURCE_MANAGER.lock().unwrap().release();
                    return Err(AtcaStatus::AtcaBadParam.to_string());
                }
            },
        );
        let mut atecc_device = AteccDevice::default();

        let iface_cfg_raw_ptr: *mut cryptoauthlib_sys::ATCAIfaceCfg = Box::into_raw(iface_cfg);
        // From now on iface_cfg is consumed and iface_cfg_ptr must be stored to be released
        // when no longer needed.

        let result = AtcaStatus::from(unsafe {
            let _guard = atecc_device
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_init(iface_cfg_raw_ptr)
        });

        atecc_device.iface_cfg_ptr = match result {
            AtcaStatus::AtcaSuccess => AtcaIfaceCfgPtrWrapper {
                ptr: iface_cfg_raw_ptr,
            },
            _ => {
                // Here init failed so no need to call a proper release
                ATECC_RESOURCE_MANAGER.lock().unwrap().release();
                unsafe { Box::from_raw(iface_cfg_raw_ptr) };
                return Err(result.to_string());
            }
        };

        // atecc_device.api_mutex is already initialized
        // from now on it is safe to call atecc_device.release();

        atecc_device.serial_number = {
            let mut number: [u8; ATCA_SERIAL_NUM_SIZE] = [0; ATCA_SERIAL_NUM_SIZE];
            let result = atecc_device.read_serial_number(&mut number);
            match result {
                AtcaStatus::AtcaSuccess => number,
                _ => {
                    atecc_device.release();
                    return Err(result.to_string());
                }
            }
        };

        atecc_device.slots = {
            let mut atca_slots = Vec::new();
            let result = atecc_device.get_config_from_chip(&mut atca_slots);
            match result {
                AtcaStatus::AtcaSuccess => atca_slots,
                _ => {
                    atecc_device.release();
                    return Err(result.to_string());
                }
            }
        };

        atecc_device.config_zone_locked = {
            match atecc_device.is_locked(ATCA_LOCK_ZONE_CONFIG) {
                Ok(is_locked) => is_locked,
                Err(err) => {
                    atecc_device.release();
                    return Err(err.to_string());
                }
            }
        };

        atecc_device.data_zone_locked = {
            match atecc_device.is_locked(ATCA_LOCK_ZONE_DATA) {
                Ok(is_locked) => is_locked,
                Err(err) => {
                    atecc_device.release();
                    return Err(err.to_string());
                }
            }
        };

        atecc_device.chip_options = {
            match atecc_device.get_chip_options_data_from_chip() {
                Ok(val) => val,
                Err(err) => {
                    atecc_device.release();
                    return Err(err.to_string());
                }
            }
        };

        let chip_type = atecc_device.get_device_type();
        let err_str = "\n\n\u{001b}[1m\u{001b}[33mcheck if 'device_type' is correct in \
        'config.toml' file, because chip on the bus seems to be";
        if atecc_device.chip_options.aes_enabled && (chip_type != AtcaDeviceType::ATECC608A) {
            atecc_device.release();
            return Err(format!(
                "{} type ATECC608x,\nand you have chosen \u{001b}[31m{}\u{001b}[33m !\u{001b}[0m\n\n",
                err_str.to_string(),
                chip_type.to_string()
            ));
        }
        if !atecc_device.chip_options.aes_enabled && (chip_type == AtcaDeviceType::ATECC608A) {
            atecc_device.release();
            return Err(format!(
                "{} of a different type than the \u{001b}[31mATECC608x\u{001b}[33m you selected !\u{001b}[0m\n\n",
                err_str.to_string()
            ));
        }

        Ok(atecc_device)
    } // AteccDevice::new()

    /// Request ATECC to generate a vector of random bytes
    fn random(&self, rand_out: &mut Vec<u8>) -> AtcaStatus {
        if self.check_that_configuration_is_not_locked(false) {
            return AtcaStatus::AtcaNotLocked;
        }
        rand_out.resize(ATCA_RANDOM_BUFFER_SIZE, 0);
        AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_random(rand_out.as_mut_ptr())
        })
    } // AteccDevice::random()

    /// Request ATECC to compute a message hash (SHA256)
    fn sha(&self, message: Vec<u8>, digest: &mut Vec<u8>) -> AtcaStatus {
        if self.check_that_configuration_is_not_locked(false) {
            return AtcaStatus::AtcaNotLocked;
        }
        let length: u16 = match u16::try_from(message.len()) {
            Ok(val) => val,
            Err(_) => return AtcaStatus::AtcaBadParam,
        };

        digest.resize(ATCA_SHA2_256_DIGEST_SIZE, 0);

        AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_sha(length, message.as_ptr(), digest.as_mut_ptr())
        })
    } // AteccDevice::sha()

    /// Execute a Nonce command in pass-through mode to load one of the
    /// device's internal buffers with a fixed value.
    /// For the ATECC608A, available targets are TempKey (32 or 64 bytes), Message
    /// Digest Buffer (32 or 64 bytes), or the Alternate Key Buffer (32 bytes). For
    /// all other devices, only TempKey (32 bytes) is available.
    fn nonce(&self, target: NonceTarget, data: &[u8]) -> AtcaStatus {
        if (self.get_device_type() != AtcaDeviceType::ATECC608A) && (target != NonceTarget::TempKey)
        {
            return AtcaStatus::AtcaBadParam;
        }
        let dev_type_608: bool = AtcaDeviceType::ATECC608A == self.get_device_type();
        let alt_key_buff: bool = NonceTarget::AltKeyBuf == target;
        let no_len_32: bool = data.len() != ATCA_NONCE_SIZE;
        let no_len_64: bool = data.len() != (2 * ATCA_NONCE_SIZE);

        if !dev_type_608 && alt_key_buff
            || alt_key_buff && no_len_32
            || !dev_type_608 && !no_len_64
            || no_len_32 && no_len_64
            || !no_len_32 && !no_len_64
        {
            return AtcaStatus::AtcaInvalidSize;
        }
        AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_nonce_load(target as u8, data.as_ptr(), data.len() as u16)
        })
    } // AteccDevice::nonce()

    /// Execute a Nonce command to generate a random nonce combining a host
    /// nonce and a device random number.
    fn nonce_rand(&self, host_nonce: &[u8], rand_out: &mut Vec<u8>) -> AtcaStatus {
        if host_nonce.len() != ATCA_NONCE_NUMIN_SIZE {
            return AtcaStatus::AtcaInvalidSize;
        }

        rand_out.resize(ATCA_RANDOM_BUFFER_SIZE, 0);

        AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_nonce_rand(host_nonce.as_ptr(), rand_out.as_mut_ptr())
        })
    } // AteccDevice::nonce_rand()

    /// Request ATECC to generate a cryptographic key
    fn gen_key(&self, key_type: KeyType, slot_id: u8) -> AtcaStatus {
        if self.check_that_configuration_is_not_locked(false) {
            return AtcaStatus::AtcaNotLocked;
        }

        if let Err(err) = self.encryption_key_setup_parameters_check(key_type, slot_id) {
            return err;
        }

        let slot = match slot_id {
            ATCA_ATECC_SLOTS_COUNT => ATCA_ATECC_TEMPKEY_KEYID,
            _ => slot_id as u16,
        };

        match key_type {
            KeyType::P256EccKey => {
                if !self.slots[slot_id as usize].config.is_secret {
                    return AtcaStatus::AtcaBadParam;
                }
                AtcaStatus::from(unsafe {
                    let _guard = self
                        .api_mutex
                        .lock()
                        .expect("Could not lock atcab API mutex");
                    cryptoauthlib_sys::atcab_genkey(slot, ptr::null_mut() as *mut u8)
                })
            }
            KeyType::Aes => {
                let mut key: Vec<u8> = Vec::with_capacity(ATCA_RANDOM_BUFFER_SIZE);
                let result = self.random(&mut key);
                if AtcaStatus::AtcaSuccess != self.random(&mut key) {
                    return result;
                }
                if key.len() > ATCA_AES_KEY_SIZE {
                    key.truncate(ATCA_AES_KEY_SIZE);
                }
                if key.len() < ATCA_BLOCK_SIZE {
                    key.resize(ATCA_BLOCK_SIZE, 0);
                }
                if slot != ATCA_ATECC_TEMPKEY_KEYID {
                    const BLOCK_IDX: u8 = 0;
                    const OFFSET: u8 = 0;
                    match self.slots[slot_id as usize].config.write_config {
                        WriteConfig::Always => self.write_zone(
                            ATCA_ZONE_DATA,
                            slot,
                            BLOCK_IDX,
                            OFFSET,
                            &mut key,
                            ATCA_BLOCK_SIZE as u8,
                        ),
                        WriteConfig::Encrypt => {
                            let num_in: [u8; ATCA_NONCE_NUMIN_SIZE] = [0; ATCA_NONCE_NUMIN_SIZE];
                            self.write_slot_with_encryption(slot, BLOCK_IDX, &key, &num_in)
                        }
                        _ => AtcaStatus::AtcaBadParam,
                    }
                } else {
                    AtcaStatus::AtcaUnimplemented // TODO
                }
            }
            _ => AtcaStatus::AtcaBadParam,
        }
    } // AteccDevice::gen_key()

    /// Request ATECC to import a cryptographic key
    fn import_key(&self, key_type: KeyType, key_data: &[u8], slot_id: u8) -> AtcaStatus {
        if self.check_that_configuration_is_not_locked(true) {
            return AtcaStatus::AtcaNotLocked;
        }
        if let Err(err) = self.encryption_key_setup_parameters_check(key_type, slot_id) {
            return err;
        }

        if ((key_type == KeyType::Aes) && (key_data.len() != ATCA_AES_KEY_SIZE))
            || ((key_type == KeyType::P256EccKey)
                && !((key_data.len() == ATCA_ATECC_PRIV_KEY_SIZE)
                    || (key_data.len() == ATCA_ATECC_PUB_KEY_SIZE)))
        {
            return AtcaStatus::AtcaInvalidSize;
        }

        let slot = match slot_id {
            ATCA_ATECC_SLOTS_COUNT => ATCA_ATECC_TEMPKEY_KEYID,
            _ => slot_id as u16,
        };

        match key_type {
            KeyType::P256EccKey => match key_data.len() {
                ATCA_ATECC_PUB_KEY_SIZE => {
                    if slot_id < ATCA_ATECC_MIN_SLOT_IDX_FOR_PUB_KEY {
                        return AtcaStatus::AtcaInvalidId;
                    }

                    AtcaStatus::from(unsafe {
                        let _guard = self
                            .api_mutex
                            .lock()
                            .expect("Could not lock atcab API mutex");
                        cryptoauthlib_sys::atcab_write_pubkey(slot, key_data.as_ptr())
                    })
                }
                _ => {
                    let mut temp_key: Vec<u8> = vec![0; 4];
                    temp_key.extend_from_slice(key_data);

                    if let Some(write_key_idx) = self.get_write_key_idx(slot_id as u8) {
                        let mut write_key = vec![0; ATCA_KEY_SIZE];
                        let result = self.get_access_key(write_key_idx, &mut write_key);

                        if AtcaStatus::AtcaSuccess == result {
                            let mut num_in: [u8; ATCA_NONCE_NUMIN_SIZE] =
                                [0; ATCA_NONCE_NUMIN_SIZE];

                            AtcaStatus::from(unsafe {
                                let _guard = self
                                    .api_mutex
                                    .lock()
                                    .expect("Could not lock atcab API mutex");
                                cryptoauthlib_sys::atcab_priv_write(
                                    slot,
                                    temp_key.as_ptr(),
                                    write_key_idx as u16,
                                    write_key.as_ptr(),
                                    num_in.as_mut_ptr(),
                                )
                            })
                        } else {
                            result
                        }
                    } else {
                        AtcaStatus::AtcaBadParam
                    }
                }
            },
            KeyType::Aes => {
                let mut temp_key: Vec<u8> = key_data.to_vec();
                temp_key.resize(ATCA_BLOCK_SIZE, 0);

                if slot != ATCA_ATECC_TEMPKEY_KEYID {
                    const BLOCK_IDX: u8 = 0;
                    const OFFSET: u8 = 0;

                    match self.slots[slot as usize].config.write_config {
                        WriteConfig::Always => self.write_zone(
                            ATCA_ZONE_DATA,
                            slot,
                            BLOCK_IDX,
                            OFFSET,
                            &mut temp_key,
                            ATCA_BLOCK_SIZE as u8,
                        ),
                        WriteConfig::Encrypt => {
                            let num_in: [u8; ATCA_NONCE_NUMIN_SIZE] = [0; ATCA_NONCE_NUMIN_SIZE];
                            self.write_slot_with_encryption(slot, BLOCK_IDX, &temp_key, &num_in)
                        }
                        _ => AtcaStatus::AtcaBadParam,
                    }
                } else {
                    self.nonce(NonceTarget::TempKey, &temp_key)
                }
            }
            KeyType::ShaOrText => AtcaStatus::AtcaUnimplemented,
            _ => AtcaStatus::AtcaBadParam,
        }
    } // AteccDevice::import_key()

    /// Request ATECC to export a cryptographic key.
    /// For key type 'ShaOrText', the amount of data returned is as large as
    /// size of the given buffer 'key_data', but when this size is greater than
    /// maximum amount of data that can be hold by slot, this function will return an error.
    /// For other types of keys, the amount of data returned corresponds to size of a given key.
    fn export_key(&self, key_type: KeyType, key_data: &mut Vec<u8>, slot_id: u8) -> AtcaStatus {
        if self.check_that_configuration_is_not_locked(true) {
            return AtcaStatus::AtcaNotLocked;
        }
        if slot_id >= ATCA_ATECC_SLOTS_COUNT {
            return AtcaStatus::AtcaInvalidId;
        };
        match key_type {
            KeyType::P256EccKey => self.get_public_key(slot_id, key_data),
            KeyType::Aes => self.read_aes_key_from_slot(slot_id, key_data),
            KeyType::ShaOrText => self.read_sha_or_text_key_from_slot(slot_id, key_data),
            _ => AtcaStatus::AtcaBadParam,
        }
    } // AteccDevice::export_key()

    /// Depending on the socket configuration, this function calculates
    /// public key based on an existing private key in the socket
    /// or exports the public key directly
    fn get_public_key(&self, slot_id: u8, public_key: &mut Vec<u8>) -> AtcaStatus {
        if self.check_that_configuration_is_not_locked(true) {
            return AtcaStatus::AtcaNotLocked;
        }
        if self.slots[slot_id as usize].config.key_type != KeyType::P256EccKey {
            return AtcaStatus::AtcaBadParam;
        }
        public_key.resize(ATCA_ATECC_PUB_KEY_SIZE, 0);

        if self.slots[slot_id as usize].config.is_secret {
            if self.slots[slot_id as usize].config.pub_info
                && self.slots[slot_id as usize].config.ecc_key_attr.is_private
            {
                AtcaStatus::from(unsafe {
                    let _guard = self
                        .api_mutex
                        .lock()
                        .expect("Could not lock atcab API mutex");
                    cryptoauthlib_sys::atcab_get_pubkey(slot_id as u16, public_key.as_mut_ptr())
                })
            } else if self.slots[slot_id as usize].config.read_key.encrypt_read {
                if slot_id < ATCA_ATECC_MIN_SLOT_IDX_FOR_PUB_KEY {
                    AtcaStatus::AtcaInvalidId
                } else {
                    // TODO encrypt read
                    // Question is whether someone will store public key in a slot that requires encrypted access?

                    AtcaStatus::AtcaUnimplemented
                }
            } else {
                AtcaStatus::AtcaBadParam
            }
        } else if self.slots[slot_id as usize].config.write_config == WriteConfig::Always {
            if slot_id < ATCA_ATECC_MIN_SLOT_IDX_FOR_PUB_KEY {
                AtcaStatus::AtcaInvalidId
            } else {
                AtcaStatus::from(unsafe {
                    let _guard = self
                        .api_mutex
                        .lock()
                        .expect("Could not lock atcab API mutex");
                    cryptoauthlib_sys::atcab_read_pubkey(slot_id as u16, public_key.as_mut_ptr())
                })
            }
        } else {
            AtcaStatus::AtcaBadParam
        }
    } // AteccDevice::get_public_key()

    /// Request ATECC to generate an ECDSA signature
    fn sign_hash(&self, mode: SignMode, slot_id: u8, signature: &mut Vec<u8>) -> AtcaStatus {
        if self.check_that_configuration_is_not_locked(true) {
            return AtcaStatus::AtcaNotLocked;
        }
        if slot_id >= ATCA_ATECC_SLOTS_COUNT {
            return AtcaStatus::AtcaInvalidId;
        }
        signature.resize(ATCA_SIG_SIZE, 0);
        match mode {
            // Executes Sign command, to sign a 32-byte external message using the
            // private key in the specified slot. The message to be signed
            // will be loaded into the Message Digest Buffer to the
            // ATECC608A device or TempKey for other devices.
            SignMode::External(hash) => AtcaStatus::from(unsafe {
                let _guard = self
                    .api_mutex
                    .lock()
                    .expect("Could not lock atcab API mutex");
                cryptoauthlib_sys::atcab_sign(slot_id as u16, hash.as_ptr(), signature.as_mut_ptr())
            }),
            _ => AtcaStatus::AtcaUnimplemented,
        }
    } // AteccDevice::sign_hash()

    /// Request ATECC to verify ECDSA signature
    fn verify_hash(
        &self,
        mode: VerifyMode,
        hash: &[u8],
        signature: &[u8],
    ) -> Result<bool, AtcaStatus> {
        if self.check_that_configuration_is_not_locked(true) {
            return Err(AtcaStatus::AtcaNotLocked);
        }
        if (signature.len() != ATCA_SIG_SIZE) || (hash.len() != ATCA_SHA2_256_DIGEST_SIZE) {
            return Err(AtcaStatus::AtcaInvalidSize);
        };
        let mut is_verified: bool = false;
        let result: AtcaStatus;

        match mode {
            // Executes the Verify command, which verifies a signature (ECDSA
            // verify operation) with a public key stored in the device. The
            // message to be signed will be loaded into the Message Digest Buffer
            // to the ATECC608A device or TempKey for other devices.
            VerifyMode::Internal(slot_number) => {
                if slot_number >= ATCA_ATECC_SLOTS_COUNT {
                    return Err(AtcaStatus::AtcaInvalidId);
                }
                result = AtcaStatus::from(unsafe {
                    let _guard = self
                        .api_mutex
                        .lock()
                        .expect("Could not lock atcab API mutex");
                    cryptoauthlib_sys::atcab_verify_stored(
                        hash.as_ptr(),
                        signature.as_ptr(),
                        slot_number as u16,
                        &mut is_verified,
                    )
                })
            }
            // Executes the Verify command, which verifies a signature (ECDSA
            // verify operation) with all components (message, signature, and
            // public key) supplied. The message to be signed will be loaded into
            // the Message Digest Buffer to the ATECC608A device or TempKey for
            // other devices.
            VerifyMode::External(public_key) => {
                if public_key.len() != ATCA_ATECC_PUB_KEY_SIZE {
                    return Err(AtcaStatus::AtcaInvalidId);
                }
                result = AtcaStatus::from(unsafe {
                    let _guard = self
                        .api_mutex
                        .lock()
                        .expect("Could not lock atcab API mutex");
                    cryptoauthlib_sys::atcab_verify_extern(
                        hash.as_ptr(),
                        signature.as_ptr(),
                        public_key.as_ptr(),
                        &mut is_verified,
                    )
                })
            }
            _ => return Err(AtcaStatus::AtcaUnimplemented),
        }

        match result {
            AtcaStatus::AtcaSuccess => Ok(is_verified),
            _ => Err(result),
        }
    } // AteccDevice::verify_hash()

    /// Data encryption function in AES unauthenticated cipher alhorithms modes
    fn cipher_encrypt(
        &self,
        algorithm: CipherAlgorithm,
        slot_id: u8,
        data: &mut Vec<u8>,
    ) -> AtcaStatus {
        if self.check_that_configuration_is_not_locked(true) {
            return AtcaStatus::AtcaNotLocked;
        }
        if !self.is_aes_enabled() {
            // If chip does not support AES hardware encryption, the operation cannot be performed
            return AtcaStatus::AtcaBadParam;
        }

        match algorithm {
            CipherAlgorithm::Ctr(cipher_param) => self.cipher_aes_ctr(cipher_param, slot_id, data),
            CipherAlgorithm::Cfb(cipher_param) => {
                self.cipher_aes_cfb(cipher_param, slot_id, data, CipherOperation::Encrypt)
            }
            CipherAlgorithm::Ofb(cipher_param) => {
                self.cipher_aes_ofb(cipher_param, slot_id, data, CipherOperation::Encrypt)
            }
            CipherAlgorithm::Ecb(cipher_param) => {
                self.cipher_aes_ecb(cipher_param, slot_id, data, CipherOperation::Encrypt)
            }
            CipherAlgorithm::Cbc(cipher_param) => {
                self.cipher_aes_cbc(cipher_param, slot_id, data, CipherOperation::Encrypt)
            }
            CipherAlgorithm::CbcPkcs7(cipher_param) => {
                self.cipher_aes_cbc_pkcs7(cipher_param, slot_id, data, CipherOperation::Encrypt)
            }
            _ => AtcaStatus::AtcaUnimplemented,
        }
    } // AteccDevice::cipher_encrypt()

    /// Data decryption function in AES unauthenticated cipher alhorithms modes
    fn cipher_decrypt(
        &self,
        algorithm: CipherAlgorithm,
        slot_id: u8,
        data: &mut Vec<u8>,
    ) -> AtcaStatus {
        if self.check_that_configuration_is_not_locked(true) {
            return AtcaStatus::AtcaNotLocked;
        }
        if !self.is_aes_enabled() {
            // If chip does not support AES hardware encryption, the operation cannot be performed
            return AtcaStatus::AtcaBadParam;
        }

        match algorithm {
            CipherAlgorithm::Ctr(cipher_param) => self.cipher_aes_ctr(cipher_param, slot_id, data),
            CipherAlgorithm::Cfb(cipher_param) => {
                self.cipher_aes_cfb(cipher_param, slot_id, data, CipherOperation::Decrypt)
            }
            CipherAlgorithm::Ofb(cipher_param) => {
                self.cipher_aes_ofb(cipher_param, slot_id, data, CipherOperation::Decrypt)
            }
            CipherAlgorithm::Ecb(cipher_param) => {
                self.cipher_aes_ecb(cipher_param, slot_id, data, CipherOperation::Decrypt)
            }
            CipherAlgorithm::Cbc(cipher_param) => {
                self.cipher_aes_cbc(cipher_param, slot_id, data, CipherOperation::Decrypt)
            }
            CipherAlgorithm::CbcPkcs7(cipher_param) => {
                self.cipher_aes_cbc_pkcs7(cipher_param, slot_id, data, CipherOperation::Decrypt)
            }
            _ => AtcaStatus::AtcaUnimplemented,
        }
    } // AteccDevice::cipher_decrypt()

    /// Data encryption function in AES AEAD (authenticated encryption with associated data) modes
    fn aead_encrypt(
        &self,
        algorithm: AeadAlgorithm,
        slot_id: u8,
        data: &mut Vec<u8>,
    ) -> Result<Vec<u8>, AtcaStatus> {
        if self.check_that_configuration_is_not_locked(true) {
            return Err(AtcaStatus::AtcaNotLocked);
        }
        if !self.is_aes_enabled() {
            // If chip does not support AES hardware encryption, the operation cannot be performed
            return Err(AtcaStatus::AtcaBadParam);
        }

        match algorithm {
            AeadAlgorithm::Ccm(aead_param) => self.encrypt_aes_ccm(aead_param, slot_id, data),
            AeadAlgorithm::Gcm(aead_param) => self.encrypt_aes_gcm(aead_param, slot_id, data),
        }
    } // AteccDevice::aead_encrypt()

    /// Data decryption function in AES AEAD (authenticated encryption with associated data) modes
    fn aead_decrypt(
        &self,
        algorithm: AeadAlgorithm,
        slot_id: u8,
        data: &mut Vec<u8>,
    ) -> Result<bool, AtcaStatus> {
        if self.check_that_configuration_is_not_locked(true) {
            return Err(AtcaStatus::AtcaNotLocked);
        }
        if !self.is_aes_enabled() {
            // If chip does not support AES hardware encryption, the operation cannot be performed
            return Err(AtcaStatus::AtcaBadParam);
        }

        match algorithm {
            AeadAlgorithm::Ccm(aead_param) => self.decrypt_aes_ccm(aead_param, slot_id, data),
            AeadAlgorithm::Gcm(aead_param) => self.decrypt_aes_gcm(aead_param, slot_id, data),
        }
    } // AteccDevice::aead_decrypt()

    /// Request ATECC to return own device type
    fn get_device_type(&self) -> AtcaDeviceType {
        AtcaDeviceType::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_get_device_type()
        })
    } // AteccDevice::get_device_type()

    /// Returns a structure containing configuration data read from ATECC
    /// during initialization of the AteccDevice object.
    fn get_config(&self, atca_slots: &mut Vec<AtcaSlot>) -> AtcaStatus {
        atca_slots.clear();
        for idx in 0..self.slots.len() {
            atca_slots.push(self.slots[idx])
        }
        AtcaStatus::AtcaSuccess
    } // AteccDevice::get_config()

    /// Command accesses some static or dynamic information from the ATECC chip
    fn info_cmd(&self, command: InfoCmdType) -> Result<Vec<u8>, AtcaStatus> {
        let mut out_data: Vec<u8> = vec![0; 4];
        let param2 = 0;
        match command {
            InfoCmdType::Revision => (),
            InfoCmdType::State => (),
            _ => return Err(AtcaStatus::AtcaUnimplemented),
        }
        let result = AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_info_base(command as u8, param2, out_data.as_mut_ptr())
        });
        match result {
            AtcaStatus::AtcaSuccess => Ok(out_data),
            _ => Err(result),
        }
    } // AteccDevice::info_cmd()

    /// A function that adds an access key for securely reading or writing data
    /// that is located in a specific slot on the ATECCx08 chip.
    /// Data is not written to the ATECCx08 chip, but to the AteccDevice structure.
    fn add_access_key(&self, slot_id: u8, access_key: &[u8]) -> AtcaStatus {
        if let Err(err) = self.access_key_setup_parameters_check(slot_id) {
            return err;
        };

        if access_key.len() != ATCA_KEY_SIZE {
            return AtcaStatus::AtcaInvalidSize;
        }

        let access_keys_mutex = self
            .access_keys
            .lock()
            .expect("Could not lock 'access_keys' mutex");

        let access_keys_obj = access_keys_mutex.try_borrow_mut();

        match access_keys_obj {
            Err(_) => AtcaStatus::AtcaFuncFail,
            Ok(mut access_keys) => {
                let mut key_arr: [u8; ATCA_KEY_SIZE] = [0; ATCA_KEY_SIZE];
                key_arr.copy_from_slice(&access_key[0..]);
                access_keys.insert(slot_id, key_arr);
                AtcaStatus::AtcaSuccess
            }
        }
    } // AteccDevice::add_access_key()

    /// A function that deletes all access keys for secure read or write operations
    /// performed by the ATECCx08 chip
    fn flush_access_keys(&self) -> AtcaStatus {
        let access_keys_mutex = self
            .access_keys
            .lock()
            .expect("Could not lock 'access_keys' mutex");

        let access_keys_obj = access_keys_mutex.try_borrow_mut();

        match access_keys_obj {
            Err(_) => AtcaStatus::AtcaFuncFail,
            Ok(mut access_keys) => {
                access_keys.clear();
                access_keys.shrink_to_fit();
                AtcaStatus::AtcaSuccess
            }
        }
    } // AteccDevice::flush_access_keys()

    /// ATECC device instance destructor
    // Requests:
    // 1. Internal rust-cryptoauthlib resource manager to release structure instance
    // 2. The structure itself to free the heap allocacted data
    // 3. CryptoAuthLib to release the ATECC device
    fn release(&self) -> AtcaStatus {
        if !ATECC_RESOURCE_MANAGER.lock().unwrap().release() {
            return AtcaStatus::AtcaBadParam;
        }
        AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            // Restore iface_cfg from iface_cfg_ptr for the boxed structure to be released
            // at the end.
            Box::from_raw(self.iface_cfg_ptr.ptr);
            cryptoauthlib_sys::atcab_release()
        })
    } // AteccDevice::release()

    //--------------------------------------------------
    //
    // Functions available only during testing
    //
    //--------------------------------------------------

    /// A generic function that reads data from the chip
    fn read_zone(
        &self,
        zone: u8,
        slot: u16,
        block: u8,
        offset: u8,
        data: &mut Vec<u8>,
        len: u8,
    ) -> AtcaStatus {
        data.resize(len as usize, 0);

        AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_read_zone(zone, slot, block, offset, data.as_mut_ptr(), len)
        })
    } // AteccDevice::read_zone()

    /// Request ATECC to read and return own configuration zone.
    /// Note: this function returns raw data, function get_config(..) implements a more
    /// structured return value.
    fn read_config_zone(&self, config_data: &mut Vec<u8>) -> AtcaStatus {
        config_data.resize(self.get_config_buffer_size(), 0);

        AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_read_config_zone(config_data.as_mut_ptr())
        })
    } // AteccDevice::read_config_zone()

    /// Compare internal config zone contents vs. config_data.
    /// Diagnostic function.
    #[allow(dead_code)]
    fn cmp_config_zone(&self, config_data: &mut [u8]) -> Result<bool, AtcaStatus> {
        let buffer_size = self.get_config_buffer_size();
        if config_data.len() != buffer_size {
            return Err(AtcaStatus::AtcaBadParam);
        }
        let mut same_config: bool = false;
        let result = AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_cmp_config_zone(config_data.as_mut_ptr(), &mut same_config)
        });
        if AtcaStatus::AtcaSuccess == result {
            Ok(same_config)
        } else {
            Err(result)
        }
    } // AteccDevice::cmp_config_zone()

    /// A function that takes an access key for securely reading or writing data
    /// that is located in a specific slot on an ATECCx08 chip.
    /// Data is not taken directly from the ATECCx08 chip, but from the AteccDevice structure
    fn get_access_key(&self, slot_id: u8, key: &mut Vec<u8>) -> AtcaStatus {
        if let Err(err) = self.access_key_setup_parameters_check(slot_id) {
            return err;
        };

        key.resize(ATCA_KEY_SIZE, 0);

        let access_keys_mutex = self
            .access_keys
            .lock()
            .expect("Could not lock 'access_keys' mutex");

        let access_keys_obj = access_keys_mutex.try_borrow_mut();

        match access_keys_obj {
            Err(_) => AtcaStatus::AtcaFuncFail,
            Ok(access_keys) => match access_keys.get(&slot_id) {
                None => AtcaStatus::AtcaInvalidId,
                Some(access_key) => {
                    *key = access_key.to_vec();
                    AtcaStatus::AtcaSuccess
                }
            },
        }
    } // AteccDevice::get_access_key()

    // ---------------------------------------------------------------
    // Private functions
    // ---------------------------------------------------------------

    /// Function that reads a key of the 'Aes' type from the indicated slot
    fn read_aes_key_from_slot(&self, slot_id: u8, key: &mut Vec<u8>) -> AtcaStatus {
        const BLOCK_IDX: u8 = 0;
        const OFFSET: u8 = 0;

        let slot_data = self.slots[slot_id as usize].config;
        if KeyType::Aes != slot_data.key_type {
            return AtcaStatus::AtcaBadParam;
        }

        let mut data_block: [u8; ATCA_BLOCK_SIZE] = [0; ATCA_BLOCK_SIZE];
        let result: AtcaStatus;

        if slot_data.is_secret && slot_data.read_key.encrypt_read {
            let num_in: [u8; ATCA_NONCE_NUMIN_SIZE] = [0; ATCA_NONCE_NUMIN_SIZE];
            result =
                self.read_slot_with_encryption(slot_id as u16, BLOCK_IDX, &mut data_block, &num_in);
        } else {
            result = self.read_zone(
                ATCA_ZONE_DATA,
                slot_id as u16,
                BLOCK_IDX,
                OFFSET,
                &mut data_block.to_vec(),
                ATCA_BLOCK_SIZE as u8,
            );
        }
        if AtcaStatus::AtcaSuccess == result {
            *key = data_block.to_vec();
            key.resize(ATCA_AES_KEY_SIZE, 0);
        }

        result
    } // AteccDevice::read_aes_key_from_slot()

    /// Function that reads a key of the 'ShaOrText' type from the indicated slot
    fn read_sha_or_text_key_from_slot(&self, slot_id: u8, key: &mut Vec<u8>) -> AtcaStatus {
        let slot_data = self.slots[slot_id as usize].config;
        if KeyType::ShaOrText != slot_data.key_type {
            return AtcaStatus::AtcaBadParam;
        }
        if key.len() > self.get_slot_capacity(slot_id).bytes as usize {
            return AtcaStatus::AtcaInvalidSize;
        }

        AtcaStatus::AtcaUnimplemented
    } // AteccDevice::read_sha_or_text_key_from_slot()

    /// A helper function for the gen_key() and import_key() methods,
    /// pre-checking combinations of input parameters
    fn encryption_key_setup_parameters_check(
        &self,
        key_type: KeyType,
        slot_id: u8,
    ) -> Result<(), AtcaStatus> {
        if slot_id > ATCA_ATECC_SLOTS_COUNT {
            return Err(AtcaStatus::AtcaInvalidId);
        }
        // First condition is a special situation when
        // an AES key can be generated in an ATECC TempKey slot.
        if ((slot_id == ATCA_ATECC_SLOTS_COUNT) && (key_type != KeyType::Aes))
            || ((key_type == KeyType::Aes) && !self.chip_options.aes_enabled)
            || ((slot_id < ATCA_ATECC_SLOTS_COUNT)
                && (key_type != self.slots[slot_id as usize].config.key_type))
        {
            return Err(AtcaStatus::AtcaBadParam);
        }
        Ok(())
    } // AteccDevice::encryption_key_setup_parameters_check()

    /// A helper function for the add_access_key() and get_access_key()
    /// methods, pre-checking combinations of input parameters
    fn access_key_setup_parameters_check(&self, slot_id: u8) -> Result<(), AtcaStatus> {
        if (slot_id > ATCA_ATECC_SLOTS_COUNT) ||
            // special condition for the key encrypting IO transmission between host and cryptochip 
            ((slot_id == ATCA_ATECC_SLOTS_COUNT) &&
            (self.get_device_type() != AtcaDeviceType::ATECC608A))
        {
            return Err(AtcaStatus::AtcaInvalidId);
        }
        Ok(())
    } // AteccDevice::access_key_setup_parameters_check()

    /// A helper function that returns number of blocks and bytes of data
    /// available for a given socket
    fn get_slot_capacity(&self, slot_id: u8) -> AtcaSlotCapacity {
        let mut slot_capacity: AtcaSlotCapacity = Default::default();
        match slot_id {
            0x00..=0x07 => {
                slot_capacity.blocks = 2;
                slot_capacity.last_block_bytes = 4;
                slot_capacity.bytes = 36;
            }
            0x08 => {
                slot_capacity.blocks = 13;
                slot_capacity.last_block_bytes = 32;
                slot_capacity.bytes = 416;
            }
            0x09..=0x0F => {
                slot_capacity.blocks = 3;
                slot_capacity.last_block_bytes = 8;
                slot_capacity.bytes = 72;
            }
            _ => {}
        }
        slot_capacity
    } // AteccDevice::get_slot_capacity()

    /// A helper function that returns socket index containing encryption key
    /// required for operation of encrypted write to the given socket
    /// or value 'None' when such an operation cannot be performed for the given socket
    fn get_write_key_idx(&self, slot_id: u8) -> Option<u8> {
        if slot_id < ATCA_ATECC_SLOTS_COUNT {
            let slot_data = self.slots[slot_id as usize].config;
            if slot_data.write_config == WriteConfig::Encrypt {
                Some(slot_data.write_key)
            } else {
                None
            }
        } else {
            None
        }
    } // AteccDevice::get_write_key_idx()

    /// A helper function that returns socket index containing encryption key
    /// required for operation of encrypted reading from the given socket
    /// or value 'None' when such an operation cannot be performed for the given socket
    fn get_read_key_idx(&self, slot_id: u8) -> Option<u8> {
        if slot_id < ATCA_ATECC_SLOTS_COUNT {
            let slot_data = self.slots[slot_id as usize].config;
            if slot_data.read_key.encrypt_read
                && slot_data.is_secret
                && !slot_data.ecc_key_attr.is_private
            {
                Some(slot_data.read_key.slot_number)
            } else {
                None
            }
        } else {
            None
        }
    } // AteccDevice::get_read_key_idx()

    /// A helper function that checks locking of configuration and data zones on the ATECC chip.
    #[inline]
    fn check_that_configuration_is_not_locked(&self, both: bool) -> bool {
        let mut result: bool = false;
        if (!self.data_zone_locked && both) || !self.config_zone_locked {
            result = true
        }
        result
    } // AteccDevice::check_that_configuration_is_not_locked()

    /// A function that reads the configuration zone to check if the specified zone is locked
    fn is_locked(&self, zone: u8) -> Result<bool, AtcaStatus> {
        let mut is_locked: bool = false;
        let result = AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_is_locked(zone, &mut is_locked)
        });
        match result {
            AtcaStatus::AtcaSuccess => Ok(is_locked),
            _ => Err(result),
        }
    } // AteccDevice::is_locked()

    /// A function that checks if the chip supports AES hardware encryption
    fn is_aes_supported(&self) -> Result<bool, AtcaStatus> {
        const LEN: u8 = 4;
        const OFFSET: u8 = 3;
        const INDEX_OF_AES_BYTE: usize = 1;

        let mut data: Vec<u8> = vec![0; LEN as usize];
        let read_status = self.read_zone(ATCA_ZONE_CONFIG, 0, 0, OFFSET, &mut data, LEN);

        match read_status {
            AtcaStatus::AtcaSuccess => Ok((data[INDEX_OF_AES_BYTE] & 1) != 0),
            _ => Err(read_status),
        }
    } // AteccDevice::is_aes_supported()

    /// A function that retrieves data about options supported by the ATECC chip
    fn get_chip_options_data_from_chip(&self) -> Result<ChipOptions, AtcaStatus> {
        const LEN: u8 = 4;
        const OFFSET: u8 = 22;
        const FIRST_DATA_BYTE: usize = 2;
        const SECOND_DATA_BYTE: usize = 3;
        const IO_KEY_EN_POS: u8 = 1;
        const KDF_AES_EN_POS: u8 = 2;

        let mut data: Vec<u8> = vec![0; LEN as usize];
        let mut chip_options: ChipOptions = Default::default();
        let read_status = self.read_zone(ATCA_ZONE_CONFIG, 0, 0, OFFSET, &mut data, LEN);

        match read_status {
            AtcaStatus::AtcaSuccess => {
                chip_options.io_key_enabled =
                    atcab_get_bit_value(data[FIRST_DATA_BYTE], IO_KEY_EN_POS);
                chip_options.io_key_in_slot = (data[SECOND_DATA_BYTE] >> 4) & 0b00001111;
                chip_options.kdf_aes_enabled =
                    atcab_get_bit_value(data[FIRST_DATA_BYTE], KDF_AES_EN_POS);
                chip_options.ecdh_output_protection = (data[SECOND_DATA_BYTE] & 0b00000011).into();
                chip_options.kdf_output_protection =
                    ((data[SECOND_DATA_BYTE] >> 2) & 0b00000011).into();
            }
            _ => return Err(read_status),
        }

        match self.is_aes_supported() {
            Ok(val) => chip_options.aes_enabled = val,
            Err(err) => {
                return Err(err);
            }
        }

        Ok(chip_options)
    } // AteccDevice::get_chip_options_data_from_chip()

    /// Request ATECC to read the configuration zone data and return it in a structure
    fn get_config_from_chip(&self, atca_slots: &mut Vec<AtcaSlot>) -> AtcaStatus {
        let mut config_data = Vec::new();
        let result = self.read_config_zone(&mut config_data);
        if AtcaStatus::AtcaSuccess != result {
            return result;
        }
        if config_data.len() != self.get_config_buffer_size() {
            return AtcaStatus::AtcaBadParam;
        }
        atca_slots.clear();
        atcab_get_config_from_config_zone(&config_data, atca_slots);
        AtcaStatus::AtcaSuccess
    } // AteccDevice::get_config_from_chip()

    /// Function returns size (in bytes) of the chip configuration data
    fn get_config_buffer_size(&self) -> usize {
        let device_type = self.get_device_type();
        match device_type {
            AtcaDeviceType::ATECC508A | AtcaDeviceType::ATECC608A | AtcaDeviceType::ATECC108A => {
                ATCA_ATECC_CONFIG_BUFFER_SIZE
            }
            _ => ATCA_ATSHA_CONFIG_BUFFER_SIZE,
        }
    } // AteccDevice::get_config_buffer_size()

    /// Request ATECC to read 9 byte serial number of the device from the config zone
    fn read_serial_number(&self, serial_number: &mut [u8; ATCA_SERIAL_NUM_SIZE]) -> AtcaStatus {
        AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_read_serial_number(serial_number.as_mut_ptr())
        })
    } // AteccDevice::read_serial_number()

    /// A generic function that reads encrypted data from the chip
    fn read_slot_with_encryption(
        &self,
        slot: u16,
        block: u8,
        data: &mut [u8],
        num_in: &[u8],
    ) -> AtcaStatus {
        if (data.len() != ATCA_BLOCK_SIZE) || (num_in.len() != ATCA_NONCE_NUMIN_SIZE) {
            return AtcaStatus::AtcaInvalidSize;
        }
        if (slot >= ATCA_ATECC_SLOTS_COUNT as u16)
            || (block >= self.get_slot_capacity(slot as u8).blocks)
        {
            return AtcaStatus::AtcaInvalidId;
        }

        if let Some(read_key_idx) = self.get_read_key_idx(slot as u8) {
            let mut read_key = vec![0; ATCA_KEY_SIZE];
            let result = self.get_access_key(read_key_idx, &mut read_key);

            if AtcaStatus::AtcaSuccess == result {
                AtcaStatus::from(unsafe {
                    let _guard = self
                        .api_mutex
                        .lock()
                        .expect("Could not lock atcab API mutex");
                    cryptoauthlib_sys::atcab_read_enc(
                        slot,
                        block,
                        data.as_mut_ptr(),
                        read_key.as_ptr(),
                        read_key_idx as u16,
                        num_in.as_ptr(),
                    )
                })
            } else {
                result
            }
        } else {
            AtcaStatus::AtcaBadParam
        }
    } // AteccDevice::read_slot_with_encryption()

    /// Generic function that writes data to the chip
    fn write_zone(
        &self,
        zone: u8,
        slot: u16,
        block: u8,
        offset: u8,
        data: &mut Vec<u8>,
        len: u8,
    ) -> AtcaStatus {
        data.resize(len as usize, 0);

        AtcaStatus::from(unsafe {
            let _guard = self
                .api_mutex
                .lock()
                .expect("Could not lock atcab API mutex");
            cryptoauthlib_sys::atcab_write_zone(zone, slot, block, offset, data.as_mut_ptr(), len)
        })
    } // AteccDevice::write_zone()

    /// Generic function that writes encrypted data to the chip
    fn write_slot_with_encryption(
        &self,
        slot: u16,
        block: u8,
        data: &[u8],
        num_in: &[u8],
    ) -> AtcaStatus {
        if (data.len() != ATCA_BLOCK_SIZE) || (num_in.len() != ATCA_NONCE_NUMIN_SIZE) {
            return AtcaStatus::AtcaInvalidSize;
        }
        if (slot >= ATCA_ATECC_SLOTS_COUNT as u16)
            || (block >= self.get_slot_capacity(slot as u8).blocks)
        {
            return AtcaStatus::AtcaInvalidId;
        }

        if let Some(write_key_idx) = self.get_write_key_idx(slot as u8) {
            let mut write_key = vec![0; ATCA_KEY_SIZE];
            let result = self.get_access_key(write_key_idx, &mut write_key);

            if AtcaStatus::AtcaSuccess == result {
                AtcaStatus::from(unsafe {
                    let _guard = self
                        .api_mutex
                        .lock()
                        .expect("Could not lock atcab API mutex");
                    cryptoauthlib_sys::atcab_write_enc(
                        slot,
                        block,
                        data.as_ptr(),
                        write_key.as_ptr(),
                        write_key_idx as u16,
                        num_in.as_ptr(),
                    )
                })
            } else {
                result
            }
        } else {
            AtcaStatus::AtcaBadParam
        }
    } // AteccDevice::write_slot_with_encryption()
}

// ---------------------------------------------------------------
// Free Auxiliary Functions
// ---------------------------------------------------------------

fn atcab_get_bit_value(byte: u8, bit_pos: u8) -> bool {
    if bit_pos < 8 {
        ((byte >> bit_pos) & 1) != 0
    } else {
        false
    }
}

fn atcab_get_write_config(data: u8) -> WriteConfig {
    match data & 0b00001111 {
        0 => WriteConfig::Always,
        1 => WriteConfig::PubInvalid,
        2..=3 => WriteConfig::Never,
        4..=7 => WriteConfig::Encrypt,
        8..=11 => WriteConfig::Never,
        _ => WriteConfig::Encrypt,
    }
}

fn atcab_get_key_type(data: u8) -> KeyType {
    match data & 0b00000111 {
        4 => KeyType::P256EccKey,
        6 => KeyType::Aes,
        7 => KeyType::ShaOrText,
        _ => KeyType::Rfu,
    }
}

pub fn atcab_get_config_from_config_zone(config_data: &[u8], atca_slots: &mut Vec<AtcaSlot>) {
    const IDX_SLOT_LOCKED: usize = 88;
    const IDX_SLOT_CONFIG: usize = 20;
    const IDX_KEY_CONFIG: usize = 96;
    for idx in 0..ATCA_ATECC_SLOTS_COUNT {
        let slot_cfg_pos = IDX_SLOT_CONFIG + (idx * 2) as usize;
        let key_cfg_pos = IDX_KEY_CONFIG + (idx * 2) as usize;
        let read_key_struct = ReadKey {
            encrypt_read: atcab_get_bit_value(config_data[slot_cfg_pos], 6),
            slot_number: config_data[slot_cfg_pos] & 0b00001111,
        };
        let ecc_key_attr_struct = EccKeyAttr {
            is_private: atcab_get_bit_value(config_data[key_cfg_pos], 0),
            ext_sign: atcab_get_bit_value(config_data[slot_cfg_pos], 0),
            int_sign: atcab_get_bit_value(config_data[slot_cfg_pos], 1),
            ecdh_operation: atcab_get_bit_value(config_data[slot_cfg_pos], 2),
            ecdh_secret_out: atcab_get_bit_value(config_data[slot_cfg_pos], 3),
        };
        let config_struct = SlotConfig {
            write_config: atcab_get_write_config(config_data[slot_cfg_pos + 1] >> 4),
            key_type: atcab_get_key_type(config_data[key_cfg_pos] >> 2),
            read_key: read_key_struct,
            ecc_key_attr: ecc_key_attr_struct,
            x509id: (config_data[key_cfg_pos + 1] >> 6) & 0b00000011,
            auth_key: config_data[key_cfg_pos + 1] & 0b00001111,
            write_key: config_data[slot_cfg_pos + 1] & 0b00001111,
            is_secret: atcab_get_bit_value(config_data[slot_cfg_pos], 7),
            limited_use: atcab_get_bit_value(config_data[slot_cfg_pos], 5),
            no_mac: atcab_get_bit_value(config_data[slot_cfg_pos], 4),
            persistent_disable: atcab_get_bit_value(config_data[key_cfg_pos + 1], 4),
            req_auth: atcab_get_bit_value(config_data[key_cfg_pos], 7),
            req_random: atcab_get_bit_value(config_data[key_cfg_pos], 6),
            lockable: atcab_get_bit_value(config_data[key_cfg_pos], 5),
            pub_info: atcab_get_bit_value(config_data[key_cfg_pos], 1),
        };
        let slot = AtcaSlot {
            id: idx,
            is_locked: {
                let index = IDX_SLOT_LOCKED + (idx / 8) as usize;
                let bit_position = idx % 8;
                let bit_value = (config_data[index] >> bit_position) & 1;
                bit_value != 1
            },
            config: config_struct,
        };
        atca_slots.push(slot);
    }
}
