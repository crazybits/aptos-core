
module 0xABCD::permissioned_transfer {
    use aptos_framework::aptos_account;
    use aptos_framework::fungible_asset;
    use aptos_framework::permissioned_signer;

    public entry fun fungible_transfer_only(
        source: &signer, to: address, amount: u64
    ) {
        let handle = permissioned_signer::create_permissioned_handle(source);
        let permissioned_signer = permissioned_signer::signer_from_permissioned(&handle);

        fungible_asset::grant_apt_permission(source, &permissioned_signer, amount);
        aptos_account::transfer(&permissioned_signer, to, amount);

        permissioned_signer::destroy_permissioned_handle(handle);
    }
}
