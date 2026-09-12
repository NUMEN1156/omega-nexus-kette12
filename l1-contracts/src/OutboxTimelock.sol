// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @title OutboxTimelock
/// @notice L1 landing contract for arche-omega-relayer outbox events.
///
/// The relayer's `EvmSink` submits each outbox payload as an explicit
/// `queue(bytes)` call from the configured submitter account. The contract
/// assigns a sequence id and queues the payload hash behind a fixed timelock.
/// Anyone may `execute` a queued item once its `eta` has passed by presenting
/// the original payload; the guardian may `cancel` pending items. Unknown ids,
/// hash mismatches, early execution and replays fail closed.
contract OutboxTimelock {
    error ZeroAddress();
    error NotSubmitter();
    error NotGuardian();
    error UnknownItem(uint256 id);
    error PayloadMismatch(uint256 id);
    error TimelockNotElapsed(uint256 id, uint256 eta, uint256 nowTs);
    error AlreadyExecuted(uint256 id);

    event Queued(uint256 indexed id, bytes32 indexed payloadHash, uint256 eta, address submitter, bytes payload);
    event Executed(uint256 indexed id, bytes32 indexed payloadHash, address executor);
    event Cancelled(uint256 indexed id, bytes32 indexed payloadHash, address guardian);

    struct Item {
        bytes32 payloadHash;
        uint256 eta;
        bool executed;
    }

    uint256 public immutable delay;
    address public immutable submitter;
    address public immutable guardian;
    uint256 public nextId = 1;
    mapping(uint256 => Item) private _items;

    constructor(uint256 delay_, address submitter_, address guardian_) {
        if (submitter_ == address(0) || guardian_ == address(0)) revert ZeroAddress();
        delay = delay_;
        submitter = submitter_;
        guardian = guardian_;
    }

    /// @dev No `fallback`/`receive`: raw calldata that happens to start with a
    ///      function selector must not be silently re-interpreted.
    function queue(bytes calldata payload) external returns (uint256 id) {
        if (msg.sender != submitter) revert NotSubmitter();
        id = nextId++;
        bytes32 payloadHash = keccak256(payload);
        uint256 eta = block.timestamp + delay;
        _items[id] = Item({payloadHash: payloadHash, eta: eta, executed: false});
        emit Queued(id, payloadHash, eta, msg.sender, payload);
    }

    function execute(uint256 id, bytes calldata payload) external {
        Item storage stored = _items[id];
        if (stored.eta == 0) revert UnknownItem(id);
        if (stored.executed) revert AlreadyExecuted(id);
        bytes32 payloadHash = keccak256(payload);
        if (payloadHash != stored.payloadHash) revert PayloadMismatch(id);
        if (block.timestamp < stored.eta) revert TimelockNotElapsed(id, stored.eta, block.timestamp);
        stored.executed = true;
        emit Executed(id, payloadHash, msg.sender);
    }

    function cancel(uint256 id) external {
        if (msg.sender != guardian) revert NotGuardian();
        Item memory stored = _items[id];
        if (stored.eta == 0) revert UnknownItem(id);
        if (stored.executed) revert AlreadyExecuted(id);
        delete _items[id];
        emit Cancelled(id, stored.payloadHash, msg.sender);
    }

    function item(uint256 id) external view returns (bytes32 payloadHash, uint256 eta, bool executed) {
        Item memory stored = _items[id];
        return (stored.payloadHash, stored.eta, stored.executed);
    }

    function isReady(uint256 id) external view returns (bool) {
        Item memory stored = _items[id];
        return stored.eta != 0 && !stored.executed && block.timestamp >= stored.eta;
    }
}
