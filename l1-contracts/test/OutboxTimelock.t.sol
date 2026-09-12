// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {OutboxTimelock} from "../src/OutboxTimelock.sol";

interface Vm {
    function warp(uint256) external;
    function prank(address) external;
    function expectRevert(bytes calldata) external;
    function expectEmit(bool, bool, bool, bool) external;
}

contract OutboxTimelockTest {
    Vm internal constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 internal constant DELAY = 3600;
    address internal constant GUARDIAN = address(0xBEEF);
    address internal constant RELAYER = address(0xCAFE);

    OutboxTimelock internal timelock;
    bytes internal payload = hex"7b22746f706963223a22636c61702e656d626564646e672e72657175657374227d";

    event Queued(uint256 indexed id, bytes32 indexed payloadHash, uint256 eta, address submitter, bytes payload);
    event Executed(uint256 indexed id, bytes32 indexed payloadHash, address executor);

    function setUp() public {
        vm.warp(1_000_000);
        timelock = new OutboxTimelock(DELAY, RELAYER, GUARDIAN);
    }

    function _queue(bytes memory data) internal returns (uint256 id) {
        vm.prank(RELAYER);
        return timelock.queue(data);
    }

    function test_constructorRejectsZeroAddresses() public {
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.ZeroAddress.selector));
        new OutboxTimelock(DELAY, address(0), GUARDIAN);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.ZeroAddress.selector));
        new OutboxTimelock(DELAY, RELAYER, address(0));
    }

    function test_queueEmitsAndStoresItem() public {
        vm.expectEmit(true, true, false, true);
        emit Queued(1, keccak256(payload), block.timestamp + DELAY, RELAYER, payload);
        uint256 id = _queue(payload);
        (bytes32 hash, uint256 eta, bool executed) = timelock.item(id);
        assertEq(hash, keccak256(payload));
        assertEq(eta, block.timestamp + DELAY);
        require(!executed, "must not be executed");
        assertEq(timelock.nextId(), 2);
    }

    function test_queueRejectsNonSubmitter() public {
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.NotSubmitter.selector));
        timelock.queue(payload);
        vm.prank(GUARDIAN);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.NotSubmitter.selector));
        timelock.queue(payload);
    }

    function test_rawCalldataIsRejected() public {
        vm.prank(RELAYER);
        (bool ok,) = address(timelock).call(payload);
        require(!ok, "raw calldata must revert");
        vm.prank(RELAYER);
        (ok,) = address(timelock).call("");
        require(!ok, "empty calldata must revert");
        assertEq(timelock.nextId(), 1);
    }

    function test_emptyPayloadIsQueueable() public {
        uint256 id = _queue("");
        (bytes32 hash,,) = timelock.item(id);
        assertEq(hash, keccak256(""));
    }

    function test_largeDelayDoesNotWrap() public {
        OutboxTimelock wide = new OutboxTimelock(uint256(type(uint64).max) + 1, RELAYER, GUARDIAN);
        vm.prank(RELAYER);
        uint256 id = wide.queue(payload);
        (, uint256 eta,) = wide.item(id);
        assertEq(eta, block.timestamp + uint256(type(uint64).max) + 1);
        require(!wide.isReady(id), "must not be ready");
    }

    function test_executeBeforeEtaFailsClosed() public {
        uint256 id = _queue(payload);
        uint256 eta = block.timestamp + DELAY;
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.TimelockNotElapsed.selector, id, eta, block.timestamp));
        timelock.execute(id, payload);
        vm.warp(eta - 1);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.TimelockNotElapsed.selector, id, eta, eta - 1));
        timelock.execute(id, payload);
    }

    function test_executeAfterEtaEmitsAndBlocksReplay() public {
        uint256 id = _queue(payload);
        vm.warp(block.timestamp + DELAY);
        require(timelock.isReady(id), "should be ready at eta");
        vm.expectEmit(true, true, false, true);
        emit Executed(id, keccak256(payload), address(this));
        timelock.execute(id, payload);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.AlreadyExecuted.selector, id));
        timelock.execute(id, payload);
    }

    function test_executeRejectsWrongPayload() public {
        uint256 id = _queue(payload);
        vm.warp(block.timestamp + DELAY);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.PayloadMismatch.selector, id));
        timelock.execute(id, hex"deadbeef");
    }

    function test_executeUnknownIdReverts() public {
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.UnknownItem.selector, 42));
        timelock.execute(42, payload);
    }

    function test_guardianCancelBlocksExecution() public {
        uint256 id = _queue(payload);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.NotGuardian.selector));
        timelock.cancel(id);
        vm.prank(GUARDIAN);
        timelock.cancel(id);
        vm.warp(block.timestamp + DELAY);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.UnknownItem.selector, id));
        timelock.execute(id, payload);
    }

    function testFuzz_queueExecuteRoundtrip(bytes calldata data, uint32 extra) public {
        uint256 id = _queue(data);
        uint256 eta = block.timestamp + DELAY;
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.TimelockNotElapsed.selector, id, eta, block.timestamp));
        timelock.execute(id, data);
        vm.warp(eta + extra);
        timelock.execute(id, data);
        (,, bool executed) = timelock.item(id);
        require(executed, "executed flag");
    }

    function assertEq(bytes32 a, bytes32 b) internal pure {
        require(a == b, "bytes32 mismatch");
    }

    function assertEq(uint256 a, uint256 b) internal pure {
        require(a == b, "uint mismatch");
    }
}
