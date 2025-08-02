# Distributed KV Store w/ multi-node/client & central registry
Cluster of nodes share keys by registering themselves on a central registry system. Client can query registry system for available nodes. and Get/Set KV from any of those nodes

## Checklist
✅ Custom RPC frames over tcp network for DSL and efficient & optimized operations
✅ Node fault tolerance
✅ Node replication
✅ Client-Node direct access reducing registry load and direct access
✅ Active Node list filter/query on registry on live tcp connections
❌ Distributed registry system (Currently centralized)
❌ Client can auto connect to other nodes if initial one fails
❌ KV store is replicated and keeps an ordered log of KV operations


