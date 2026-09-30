//! The frozen layout's bytes, as they were **before** the session refactor: one SHA-256
//! per seed over `layout.force.barnes_hut`'s own output columns, `f32` little-endian,
//! **all of `x` then all of `y`**, in node order.
//!
//! Captured from the pre-refactor stage, so this table is the promise `BarnesHut::run`
//! keeps: the same topology at the frozen parameters hashes the same after the layout
//! became "a default session, no pins, 112 ticks". Input per seed is the gate's own
//! model, `seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE)` — 2 to 66 nodes
//! over seeds 0..64, which is the size range where the golden-spiral seed, the quadtree's
//! subdivision and every coincidence guard are all exercised at once.
//!
//! Base commit: 8e8e93b (`origin/develop`). Regenerate from that code with
//! `git archive 8e8e93b | tar -x -C <dir>`, then print `sha256(f32le(x) ++ f32le(y))` of
//! `BarnesHut::run` per seed with `support::digest32` and diff against this table.
//!
//! Reproduce a row with `cargo test -p graph-core session::tests::m1a` and, if it fails,
//! by printing the same bytes from `BarnesHut::run` — never by editing this table
//! without saying so in the report.

/// The digests, indexed by seed.
pub(super) const FROZEN: [&str; 65] = [
    "a9959e7bfe8c60a0d31f8fa22cfde367e24e37d821a9dcd6f212b9d68027bda4", // seed 0
    "39c11968fc1f1bf37e230b1a6dd74fe18288780fed645cbd087e53516227d851", // seed 1
    "dc218eaeab81c96b349525bc5a7b8c788c25e68b30796c68127e891397002430", // seed 2
    "3d315350e571f4fdb514f6c209c2c89ba6b8b1f5aee09845f5ad6d9208c25569", // seed 3
    "959b11b15021050ed36e9b1639795eb4bb228e6878f194a15845a63ac5062608", // seed 4
    "268c6fe8ca812e6117f48c7cd481a2658ee14b3bcebfdeec4292de2832b244be", // seed 5
    "eb22cb3efcc914744b243d8110cf0d65de3db2ee538c1e17cb593fd4b4533c21", // seed 6
    "402bbebd9d55f3beef56d7d7ffd7a77caf859f53fc36eb8c4b4d8625fa8ce24f", // seed 7
    "da56131c4e95e0a36112d8e9a83eb7d5076afd10399989a43fedac847cec8671", // seed 8
    "6ae8ec6cbb97df642d81c47382bdb312a7e927bd75283dd4869b6b8d7085ee60", // seed 9
    "85f7ce7e60b8344a90866d27e4a12bcacfd2c2a59e1195a1c6e282a2559ccde1", // seed 10
    "bdd1b3ec0d238f5cb1f1cf121baad7c26aab02140fd5b85b5aaaf681f446f518", // seed 11
    "063cd0a4ecbaa755b93479307b8367337f25a79476e2d7dd4fd783df1720fbcf", // seed 12
    "34a4bc1a6ed60c5152cca544aadad0ea4644f08bb085bf5d2c123f2564223b12", // seed 13
    "8288651799057bf0a04954cfe3ac24c3b52517ad4f1719e233ab0a390a8f634c", // seed 14
    "a70a2d8c79d3074600f2c72611bb02b6046636e360de5d25f623ce7bf8400240", // seed 15
    "eb2561cae2f39bea6fd758e9ad029e68ef397ab281a7689300b2d34729f20edb", // seed 16
    "768aae771201ea273bd50c134a34c5db7fe84288e0091605dde28a08031f5554", // seed 17
    "7b885afdf9784db905958ed7737e0695a815806b699210bddefb6f1abcd30ca0", // seed 18
    "89d363a99026ac0553714d41baf70538363aadbfa0703dae1c9d61991aabcbcb", // seed 19
    "20693795b44262667fc3cd498a9ce72a2e29e64806159818d2fabcd45aef314f", // seed 20
    "ff9348c33985957182c462a365f201fe8a5401863fd6854abcef84dc9ac0f0e9", // seed 21
    "c2a777d519f2748f1c7329f2146e8896e638d788a80fb14f4dff8e33434ae69d", // seed 22
    "2788258d133a2cf180ba7763762a1b3e98a1793d64b5d7725c00b24528801534", // seed 23
    "adbdce85e99d3eb3eeb65c3c35b6ee0e2b6e0b30ce34e1a6af07ee2f149221b7", // seed 24
    "234ee2258a39a80f583f717707a0eaa4908464bb5d6d3e7e8346b356529cc2c0", // seed 25
    "cc6cc40024f786fdbbb7aa438e99e35472c1083e290c04e5687cd9d65883dea3", // seed 26
    "5dfd9fd22a6eed3bff3612b5fe0a6244dd40a708d3eeee6ec9f44b5189e5efff", // seed 27
    "224c5770de8b6aea65b141c8e7f14795d2ae8b0be4dcda5d39dda10920e69c3f", // seed 28
    "269102f9b34a33319d9eea741b5f9de826ed4a194e7afc64490a6b6f9b25a3e0", // seed 29
    "1e49695eead115bf26a1f037b7a4b504ad31723e57710267d9928bf1fedc45ce", // seed 30
    "5f9c3fde22e0ea45ec43f4dcedf8d7e9702921f2cb3602a981bb9c4eead6e5fa", // seed 31
    "866a53ae5e4d85709f354b0d12d7cb9c910cc0f954923f49f4c08287e5acd052", // seed 32
    "8f633ba61e3de0ffe22b873424fa4293a0bdc4faea29777a3ff9c4a1a4db1c96", // seed 33
    "269351b5a7dee762093073681179f0324148f3f02b20ada4bd35bcd9374cc75a", // seed 34
    "c00765f823ad45dc2b06542b82bebc3198715a29e9639e9a496381c161b8af88", // seed 35
    "58ae307846960becade57fa685694b34c97c7e6a51314f4646cbb57dd83d08d2", // seed 36
    "b7e86662f1a1a29d9d87b6364f488fdf1b5c2cbce952d4c1191f388ce1acb339", // seed 37
    "227e3ebc8c1f73478c663a1076e3cf5d2068e1c653d37d8612343883145d571e", // seed 38
    "4693926d0e06e9fc58931880ed7ca68017199362b8971d6d7bdfc6294ac2646c", // seed 39
    "197a80a0f0e49c2ef929dbebae1bd8db3e639296ccb7aaf31a5019202e202b2b", // seed 40
    "a7eef36eb14f43851423b8abf7e59ecc878918d14f044e4fd91453c1b567dd20", // seed 41
    "6b93c9398c240e4ec3f32f4aab4eedd809b80db36d5aec3a80efe6309d77c8cd", // seed 42
    "fc1fe6011d45c36698d2f4984fb9d10089057df8d8c46181900f76fe3d130593", // seed 43
    "f88029729ef1c56652f5191f0de6719ff8ec59554873a427a339257084a8a954", // seed 44
    "cdeee129d38388190bc42e6bd3360fb7e98cc396ea13688026edea7e709854c2", // seed 45
    "5bfc43e5513d900b8fa78ce2196f8a98854d08b94e91a2282665fdd0cb00460b", // seed 46
    "d5b9efab49492642aa40519768b844c939de9d1fb5029f868813100a69271afa", // seed 47
    "a69e1c7a07cd0fbb7a236239e700de809e5b5a0392c3ef7db56322be174d43f9", // seed 48
    "e6e83d70a106d2708857ad3096978170313782b9df06db27b3889da4ab1edca2", // seed 49
    "cb9e4039638b7bdb934d11e348d4e4b2db0bf5f09c674e908d2b02c2ea29b486", // seed 50
    "1adb7a53d89113d9f08951bb2b8694ddb7837a17dc51d70ae64d198a32b6b428", // seed 51
    "12918d05885d1c59c8c859417ef4097017888325d841affb48ce7c7742ca273e", // seed 52
    "b61482550f6abfa88b9c2a42e8a4a68d865f9099ceb9b54481d7d23c08d98674", // seed 53
    "58316bbf4bbddbd28377f1f39fd60f0465c992acae95138a72e537455f24496d", // seed 54
    "4a57b409e6f723d47b54bee69781491978ed096b36137381ba3a13d9b4c0467a", // seed 55
    "262da3e1814b9c20d593cd202184aefa87cfc89c4b9b1d4cdd2e32d4ca603342", // seed 56
    "89545fe54066cafc937c75d19375aa31e18377d0b54ff28078bbe05d9d0fcffb", // seed 57
    "34f8c372de25629b9d148c4339e7f5eb2be84d22b0d647d3f7024bc94b21ea5d", // seed 58
    "95d4c893d5bd3c59ba59efad6845ce0f39b2f2d429ff1075c392dba25cf55831", // seed 59
    "50f503476618aad178e2c2e8019ee87d7206bf78fa41fbdeb74e1c2d124d29a7", // seed 60
    "fc8440cb5a33dc6339c8f9c2aa872069f338e8a172a8c4022fce782796f4d13f", // seed 61
    "f5b0bc6a11500dbf748c47ff4647a09de5b7e5f2e6a5ab4ee49b2a71985dcf4e", // seed 62
    "1f8c59a2c9a30219d0545a0b6b647de66c28a9d347302c87ba5738ee25c24bc9", // seed 63
    "cd20053e1cbfb1c18c9401742092afa5f4ecf87444f5c7b78a6099e20fb77044", // seed 64
];
