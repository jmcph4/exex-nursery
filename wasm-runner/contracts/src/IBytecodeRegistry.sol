// SPDX-License-Identifier: MIT
pragma solidity 0.8.29;

interface IBytecodeRegistry {
    event ExecutionRequest(address sender, bytes code);
    function requestExecution(bytes calldata code) external returns (uint256);
}
