import pytest
from dag_consensus import MysticetiDAG, Block

def test_propose_block_success():
    dag = MysticetiDAG(n_validators=4, f_byzantine=1, max_round_jump=2)
    author = b"validator1"
    peer = b"validator2"
    txs = [b"tx1", b"tx2"]

    block = dag.propose_block(author, peer, txs)

    assert block.round == 1
    assert block.author == author
    assert block.peer == peer
    assert block.transactions == txs
    assert dag.current_round == 1

def test_propose_block_round_jumping_violation():
    dag = MysticetiDAG(n_validators=4, f_byzantine=1, max_round_jump=2)
    author = b"validator1"
    peer = b"validator2"
    txs = [b"tx1"]

    # Attempting to jump from round 0 to round 4 (max jump is 2)
    with pytest.raises(ValueError, match="Round-jumping bloqueado"):
        dag.propose_block(author, peer, txs, target_round=4)

def test_commit_blocks():
    dag = MysticetiDAG(n_validators=4, f_byzantine=1, max_round_jump=2)

    # Create blocks in round 1
    b1_1 = dag.propose_block(b"v1", b"peer", [b"tx"], target_round=1)
    b1_2 = dag.propose_block(b"v2", b"peer", [b"tx"], target_round=1)

    # Create blocks in round 2 referencing round 1 blocks
    b2_1 = dag.propose_block(b"v3", b"peer", [b"tx"], target_round=2, parents=[b1_1.block_id, b1_2.block_id])
    b2_2 = dag.propose_block(b"v4", b"peer", [b"tx"], target_round=2, parents=[b1_1.block_id])
    b2_3 = dag.propose_block(b"v1", b"peer", [b"tx"], target_round=2, parents=[b1_1.block_id])

    committed = dag.commit_blocks()

    # b1_1 is referenced by v3, v4, v1 (3 distinct validators >= 2f+1)
    assert b1_1.block_id in committed

    # b1_2 is referenced by v3 (1 validator < 2f+1)
    assert b1_2.block_id not in committed

def test_topological_order():
    dag = MysticetiDAG(n_validators=4, f_byzantine=1, max_round_jump=2)

    b1 = dag.propose_block(b"v1", b"peer", [b"tx1"], target_round=1)
    b2 = dag.propose_block(b"v2", b"peer", [b"tx2"], target_round=2, parents=[b1.block_id])
    b31 = dag.propose_block(b"v3", b"peer", [b"tx3"], target_round=3, parents=[b1.block_id])
    b32 = dag.propose_block(b"v4", b"peer", [b"tx4"], target_round=3, parents=[b1.block_id])
    b33 = dag.propose_block(b"v1", b"peer", [b"tx5"], target_round=3, parents=[b1.block_id])

    # Force b1 and b2 to be committed manually for testing topological order
    dag.committed_blocks = [b1.block_id, b2.block_id]

    order = dag.topological_order()
    assert len(order) == 2
    # b2 depends on b1, so b1 should come before b2
    assert order.index(b1.block_id) < order.index(b2.block_id)
