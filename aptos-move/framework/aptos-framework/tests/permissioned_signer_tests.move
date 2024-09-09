#[test_only]
module aptos_framework::permissioned_signer_tests {
    use aptos_framework::account::create_signer_for_test;
    use aptos_framework::permissioned_signer;
    use aptos_framework::timestamp;
    use std::option;
    use std::signer;

    struct OnePermission has copy, drop, store {}

    struct AddressPermission has copy, drop, store {
        addr: address
    }


    #[test(creator = @0xcafe)]
    fun test_permission_e2e(
        creator: &signer,
    ) {
        let aptos_framework = create_signer_for_test(@0x1);
        timestamp::set_time_has_started_for_testing(&aptos_framework);

        let perm_handle = permissioned_signer::create_permissioned_handle(creator);
        let perm_signer = permissioned_signer::signer_from_permissioned(&perm_handle);

        assert!(permissioned_signer::is_permissioned_signer(&perm_signer), 1);
        assert!(!permissioned_signer::is_permissioned_signer(creator), 1);
        assert!(signer::address_of(&perm_signer) == signer::address_of(creator), 1);

        permissioned_signer::authorize(creator, &perm_signer, 100, OnePermission {});
        assert!(permissioned_signer::capacity(&perm_signer, OnePermission {}) == option::some(100), 1);

        assert!(permissioned_signer::check_permission(&perm_signer, 10, OnePermission {}), 1);
        assert!(permissioned_signer::capacity(&perm_signer, OnePermission {}) == option::some(90), 1);

        permissioned_signer::authorize(creator, &perm_signer, 5, AddressPermission { addr: @0x1 });

        assert!(permissioned_signer::capacity(&perm_signer, AddressPermission { addr: @0x1 }) == option::some(5), 1);
        assert!(permissioned_signer::capacity(&perm_signer, AddressPermission { addr: @0x2 }) == option::none(), 1);

        // Not enough capacity, check permission should return false
        assert!(!permissioned_signer::check_permission(&perm_signer, 10, AddressPermission { addr: @0x1 }), 1);

        permissioned_signer::revoke_permission(&perm_signer, OnePermission {});
        assert!(permissioned_signer::capacity(&perm_signer, OnePermission {}) == option::none(), 1);

        permissioned_signer::destroy_permissioned_handle(perm_handle);
    }

    #[test(creator = @0xcafe)]
    #[expected_failure(abort_code = 0x50005, location = aptos_framework::permissioned_signer)]
    fun test_permission_expiration(
        creator: &signer,
    ) {
        let aptos_framework = create_signer_for_test(@0x1);
        timestamp::set_time_has_started_for_testing(&aptos_framework);

        let perm_handle = permissioned_signer::create_permissioned_handle(creator);
        let _perm_signer = permissioned_signer::signer_from_permissioned(&perm_handle);

        timestamp::fast_forward_seconds(60);
        let _perm_signer = permissioned_signer::signer_from_permissioned(&perm_handle);

        permissioned_signer::destroy_permissioned_handle(perm_handle);
    }

    // invalid authorization
    // 1. master signer is a permissioned signer
    // 2. permissioned signer is a master signer
    // 3. permissioned and main signer address mismatch
    #[test(creator = @0xcafe)]
    #[expected_failure(abort_code = 0x50002, location = aptos_framework::permissioned_signer)]
    fun test_auth_1(
        creator: &signer,
    ) {
        let aptos_framework = create_signer_for_test(@0x1);
        timestamp::set_time_has_started_for_testing(&aptos_framework);

        let perm_handle = permissioned_signer::create_permissioned_handle(creator);
        let perm_signer = permissioned_signer::signer_from_permissioned(&perm_handle);

        permissioned_signer::authorize(&perm_signer, &perm_signer, 100, OnePermission {});
        permissioned_signer::destroy_permissioned_handle(perm_handle);
    }

    #[test(creator = @0xcafe)]
    #[expected_failure(abort_code = 0x50002, location = aptos_framework::permissioned_signer)]
    fun test_auth_2(
        creator: &signer,
    ) {
        permissioned_signer::authorize(creator, creator, 100, OnePermission {});
    }

    #[test(creator = @0xcafe, creator2 = @0xbeef)]
    #[expected_failure(abort_code = 0x50002, location = aptos_framework::permissioned_signer)]
    fun test_auth_3(
        creator: &signer,
        creator2: &signer,
    ) {
        let aptos_framework = create_signer_for_test(@0x1);
        timestamp::set_time_has_started_for_testing(&aptos_framework);

        let perm_handle = permissioned_signer::create_permissioned_handle(creator);
        let perm_signer = permissioned_signer::signer_from_permissioned(&perm_handle);

        permissioned_signer::authorize(creator2, &perm_signer, 100, OnePermission {});
        permissioned_signer::destroy_permissioned_handle(perm_handle);
    }

    // Accessing capacity on a master signer
    #[test(creator = @0xcafe)]
    #[expected_failure(abort_code = 0x50003, location = aptos_framework::permissioned_signer)]
    fun test_invalid_capacity(
        creator: &signer,
    ) {
        permissioned_signer::capacity(creator, OnePermission {});
    }

    // creating permission using a permissioned signer
    #[test(creator = @0xcafe)]
    #[expected_failure(abort_code = 0x50001, location = aptos_framework::permissioned_signer)]
    fun test_invalid_creation(
        creator: &signer,
    ) {
        let aptos_framework = create_signer_for_test(@0x1);
        timestamp::set_time_has_started_for_testing(&aptos_framework);

        let perm_handle = permissioned_signer::create_permissioned_handle(creator);
        let perm_signer = permissioned_signer::signer_from_permissioned(&perm_handle);

        let perm_handle_2 = permissioned_signer::create_permissioned_handle(&perm_signer);
        permissioned_signer::destroy_permissioned_handle(perm_handle);
        permissioned_signer::destroy_permissioned_handle(perm_handle_2);
    }

    #[test(creator = @0xcafe)]
    fun test_permission_revokation_success(
        creator: &signer,
    ) {
        let aptos_framework = create_signer_for_test(@0x1);
        timestamp::set_time_has_started_for_testing(&aptos_framework);

        let perm_handle = permissioned_signer::create_permissioned_handle(creator);
        let _perm_signer = permissioned_signer::signer_from_permissioned(&perm_handle);

        permissioned_signer::revoke_permission_handle(creator, permissioned_signer::permission_address(&perm_handle));

        permissioned_signer::destroy_permissioned_handle(perm_handle);
    }

    #[test(creator = @0xcafe)]
    #[expected_failure(abort_code = 0x50007, location = aptos_framework::permissioned_signer)]
    fun test_permission_revokation_and_access(
        creator: &signer,
    ) {
        let aptos_framework = create_signer_for_test(@0x1);
        timestamp::set_time_has_started_for_testing(&aptos_framework);

        let perm_handle = permissioned_signer::create_permissioned_handle(creator);
        let _perm_signer = permissioned_signer::signer_from_permissioned(&perm_handle);

        permissioned_signer::revoke_permission_handle(creator, permissioned_signer::permission_address(&perm_handle));
        let _perm_signer = permissioned_signer::signer_from_permissioned(&perm_handle);

        permissioned_signer::destroy_permissioned_handle(perm_handle);
    }
}
