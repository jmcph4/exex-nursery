// SPDX-License-Identifier: MIT
pragma solidity 0.8.29;

import "../lib/solady/src/auth/Ownable.sol";

import "./IBytecodeRegistry.sol";

contract PermissionedBytecodeRegistry is IBytecodeRegistry, Ownable {
    uint256 public numPrograms;
    mapping (uint256 => bytes) public programs;

    function requestExecution(bytes calldata code) public onlyOwner returns (uint256) {
        programs[numPrograms] = code;
        numPrograms += 1;
        emit ExecutionRequest(msg.sender, code);
        return numPrograms - 1;
    }
}
