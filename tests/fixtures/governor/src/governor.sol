// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

interface IVotes {
    function getVotes(address account) external view returns (uint256);
}

contract TimelockController {
    uint256 public delay;
    mapping(bytes32 => bool) public executed;
    address public admin;

    modifier onlyAdmin() {
        require(msg.sender == admin, "Not admin");
        _;
    }

    function schedule(bytes32 id, uint256 eta) external onlyAdmin {
        require(eta >= block.timestamp + delay, "ETA too soon");
    }

    function execute(bytes32 id) external {
        require(executed[id] == false, "Already executed");
        executed[id] = true;
    }
}

contract Governor is TimelockController {
    IVotes public token;
    uint256 public proposalThreshold;
    uint256 public votingPeriod;

    struct Proposal {
        address proposer;
        uint256 startBlock;
        uint256 endBlock;
        uint256 forVotes;
        uint256 againstVotes;
        bool executed;
        bool canceled;
    }

    enum ProposalState { Pending, Active, Defeated, Succeeded, Executed, Canceled }

    mapping(uint256 => Proposal) public proposals;
    mapping(uint256 => mapping(address => bool)) public hasVoted;
    uint256 public proposalCount;

    event ProposalCreated(uint256 id, address proposer);
    event VoteCast(address voter, uint256 proposalId, bool support, uint256 weight);
    event ProposalExecuted(uint256 id);

    error ProposalNotActive(uint256 proposalId);
    error AlreadyVoted(address voter, uint256 proposalId);
    error InsufficientVotingPower(uint256 required, uint256 actual);

    modifier onlyActiveProposal(uint256 proposalId) {
        require(state(proposalId) == ProposalState.Active, "Not active");
        _;
    }

    function propose() external returns (uint256) {
        require(
            token.getVotes(msg.sender) >= proposalThreshold,
            "Below threshold"
        );

        proposalCount++;
        uint256 proposalId = proposalCount;

        proposals[proposalId] = Proposal({
            proposer: msg.sender,
            startBlock: block.number + 1,
            endBlock: block.number + 1 + votingPeriod,
            forVotes: 0,
            againstVotes: 0,
            executed: false,
            canceled: false
        });

        emit ProposalCreated(proposalId, msg.sender);
        return proposalId;
    }

    function castVote(uint256 proposalId, bool support)
        external
        onlyActiveProposal(proposalId)
    {
        require(!hasVoted[proposalId][msg.sender], "Already voted");

        uint256 weight = token.getVotes(msg.sender);
        require(weight > 0, "No voting power");

        hasVoted[proposalId][msg.sender] = true;

        if (support) {
            proposals[proposalId].forVotes += weight;
        } else {
            proposals[proposalId].againstVotes += weight;
        }

        emit VoteCast(msg.sender, proposalId, support, weight);
    }

    function executeProposal(uint256 proposalId) external {
        require(state(proposalId) == ProposalState.Succeeded, "Not succeeded");

        proposals[proposalId].executed = true;
        emit ProposalExecuted(proposalId);
    }

    function cancelProposal(uint256 proposalId) external {
        Proposal storage proposal = proposals[proposalId];
        require(msg.sender == proposal.proposer, "Not proposer");
        require(!proposal.executed, "Already executed");

        proposal.canceled = true;
    }

    function state(uint256 proposalId) public view returns (ProposalState) {
        Proposal storage proposal = proposals[proposalId];

        if (proposal.canceled) {
            return ProposalState.Canceled;
        }
        if (proposal.executed) {
            return ProposalState.Executed;
        }
        if (block.number <= proposal.startBlock) {
            return ProposalState.Pending;
        }
        if (block.number <= proposal.endBlock) {
            return ProposalState.Active;
        }
        if (proposal.forVotes > proposal.againstVotes) {
            return ProposalState.Succeeded;
        }
        return ProposalState.Defeated;
    }
}
