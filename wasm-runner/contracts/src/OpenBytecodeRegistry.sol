// SPDX-License-Identifier: MIT
pragma solidity 0.8.29;

import "./IBytecodeRegistry.sol";

contract OpenBytecodeRegistry is IBytecodeRegistry {
    uint256 public numPrograms;
    mapping (uint256 => bytes) public programs;

    function requestExecution(bytes calldata code) public returns (uint256) {
        programs[numPrograms] = code;
        numPrograms += 1;
        emit ExecutionRequest(msg.sender, code);
        return numPrograms - 1;
    }
}
