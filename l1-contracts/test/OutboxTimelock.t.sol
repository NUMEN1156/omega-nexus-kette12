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
        timelock = new OutboxTimelock(DELAY, GUARDIAN);
    }

    function _queueViaFallback() internal returns (uint256 id) {
        id = timelock.nextId();
        vm.prank(RELAYER);
        (bool ok,) = address(timelock).call(payload);
        require(ok, "fallback queue failed");
    }

    function test_fallbackQueuesRawCalldata() public {
        vm.expectEmit(true, true, false, true);
        emit Queued(1, keccak256(payload), block.timestamp + DELAY, RELAYER, payload);
        uint256 id = _queueViaFallback();
        (bytes32 hash, uint64 eta, bool executed) = timelock.item(id);
        assertEq(hash, keccak256(payload));
        assertEq(eta, block.timestamp + DELAY);
        require(!executed, "must not be executed");
        assertEq(timelock.nextId(), 2);
    }

    function test_executeBeforeEtaFailsClosed() public {
        uint256 id = _queueViaFallback();
        uint256 eta = block.timestamp + DELAY;
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.TimelockNotElapsed.selector, id, eta, block.timestamp));
        timelock.execute(id, payload);
        vm.warp(eta - 1);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.TimelockNotElapsed.selector, id, eta, eta - 1));
        timelock.execute(id, payload);
    }

    function test_executeAfterEtaEmitsAndBlocksReplay() public {
        uint256 id = _queueViaFallback();
        vm.warp(block.timestamp + DELAY);
        require(timelock.isReady(id), "should be ready at eta");
        vm.expectEmit(true, true, false, true);
        emit Executed(id, keccak256(payload), address(this));
        timelock.execute(id, payload);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.AlreadyExecuted.selector, id));
        timelock.execute(id, payload);
    }

    function test_executeRejectsWrongPayload() public {
        uint256 id = _queueViaFallback();
        vm.warp(block.timestamp + DELAY);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.PayloadMismatch.selector, id));
        timelock.execute(id, hex"deadbeef");
    }

    function test_executeUnknownIdReverts() public {
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.UnknownItem.selector, 42));
        timelock.execute(42, payload);
    }

    function test_emptyPayloadRejected() public {
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.EmptyPayload.selector));
        timelock.queue("");
        (bool ok,) = address(timelock).call("");
        require(!ok, "empty calldata must revert");
    }

    function test_guardianCancelBlocksExecution() public {
        uint256 id = _queueViaFallback();
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.NotGuardian.selector));
        timelock.cancel(id);
        vm.prank(GUARDIAN);
        timelock.cancel(id);
        vm.warp(block.timestamp + DELAY);
        vm.expectRevert(abi.encodeWithSelector(OutboxTimelock.UnknownItem.selector, id));
        timelock.execute(id, payload);
    }

    function testFuzz_queueExecuteRoundtrip(bytes calldata data, uint32 extra) public {
        if (data.length == 0) return;
        uint256 id = timelock.queue(data);
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
