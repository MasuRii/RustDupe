window.BENCHMARK_DATA = {
  "lastUpdate": 1770529057663,
  "repoUrl": "https://github.com/MasuRii/RustDupe",
  "entries": {
    "RustDupe Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "kanjiharigana@gmail.com",
            "name": "MasuRii",
            "username": "MasuRii"
          },
          "committer": {
            "email": "kanjiharigana@gmail.com",
            "name": "MasuRii",
            "username": "MasuRii"
          },
          "distinct": true,
          "id": "2335ad0b19d24b3d41a32a70931a615e5fa6a4d1",
          "message": "perf(cache): enable SQLite WAL mode for better concurrency\n\nConfigure SQLite with performance optimizations:\n- Enable WAL (Write-Ahead Logging) for concurrent reads during writes\n- Set busy_timeout=5000ms to retry on temporary locks instead of failing\n- Use synchronous=NORMAL which is safe with WAL and faster\n\nThis eliminates 'database is locked' errors during high-throughput scanning\noperations where multiple threads write to the cache concurrently.",
          "timestamp": "2026-02-08T13:16:50+08:00",
          "tree_id": "2854453b66ef72b09ff55e1bf91150e9cc88c04f",
          "url": "https://github.com/MasuRii/RustDupe/commit/2335ad0b19d24b3d41a32a70931a615e5fa6a4d1"
        },
        "date": 1770528061254,
        "tool": "cargo",
        "benches": [
          {
            "name": "walker_150_files",
            "value": 522478,
            "range": "± 18265",
            "unit": "ns/iter"
          },
          {
            "name": "hasher/blake3_1KB",
            "value": 7752,
            "range": "± 15",
            "unit": "ns/iter"
          },
          {
            "name": "hasher/blake3_1024KB",
            "value": 331841,
            "range": "± 1360",
            "unit": "ns/iter"
          },
          {
            "name": "hasher/blake3_10240KB",
            "value": 3137486,
            "range": "± 117247",
            "unit": "ns/iter"
          },
          {
            "name": "perceptual_hasher/Phash",
            "value": 933668,
            "range": "± 14315",
            "unit": "ns/iter"
          },
          {
            "name": "perceptual_hasher/Dhash",
            "value": 928069,
            "range": "± 5115",
            "unit": "ns/iter"
          },
          {
            "name": "perceptual_hasher/Ahash",
            "value": 928800,
            "range": "± 2335",
            "unit": "ns/iter"
          },
          {
            "name": "pipeline_approx_80_files",
            "value": 1656028,
            "range": "± 25438",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kanjiharigana@gmail.com",
            "name": "MasuRii",
            "username": "MasuRii"
          },
          "committer": {
            "email": "kanjiharigana@gmail.com",
            "name": "MasuRii",
            "username": "MasuRii"
          },
          "distinct": true,
          "id": "84db4aae378ea7d529a305cc14f60e855558784a",
          "message": "fix(tests): stabilize path edge case tests on macOS\n\nAdd sync_all() calls to flush files to disk before scanning.\nThis fixes flaky test_paths_with_quotes and test_paths_with_newlines\non macOS CI where filesystem caching can cause timing issues.",
          "timestamp": "2026-02-08T13:20:17+08:00",
          "tree_id": "89c21843ffe1797d440476c48931d35fe5dd4a08",
          "url": "https://github.com/MasuRii/RustDupe/commit/84db4aae378ea7d529a305cc14f60e855558784a"
        },
        "date": 1770528264762,
        "tool": "cargo",
        "benches": [
          {
            "name": "walker_150_files",
            "value": 532941,
            "range": "± 12677",
            "unit": "ns/iter"
          },
          {
            "name": "hasher/blake3_1KB",
            "value": 7774,
            "range": "± 18",
            "unit": "ns/iter"
          },
          {
            "name": "hasher/blake3_1024KB",
            "value": 328779,
            "range": "± 8329",
            "unit": "ns/iter"
          },
          {
            "name": "hasher/blake3_10240KB",
            "value": 3097649,
            "range": "± 68386",
            "unit": "ns/iter"
          },
          {
            "name": "perceptual_hasher/Phash",
            "value": 954526,
            "range": "± 10056",
            "unit": "ns/iter"
          },
          {
            "name": "perceptual_hasher/Dhash",
            "value": 930567,
            "range": "± 8661",
            "unit": "ns/iter"
          },
          {
            "name": "perceptual_hasher/Ahash",
            "value": 931720,
            "range": "± 4008",
            "unit": "ns/iter"
          },
          {
            "name": "pipeline_approx_80_files",
            "value": 1683644,
            "range": "± 33785",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "kanjiharigana@gmail.com",
            "name": "MasuRii",
            "username": "MasuRii"
          },
          "committer": {
            "email": "kanjiharigana@gmail.com",
            "name": "MasuRii",
            "username": "MasuRii"
          },
          "distinct": true,
          "id": "28ff5c65655845acb66f4ca373df679bcf209446",
          "message": "fix(tui): accept '?' key with SHIFT modifier on Windows\n\nOn Windows terminals, pressing '?' sends KeyCode::Char('?') with\nKeyModifiers::SHIFT because it requires Shift+/ to type. Some terminals\nreport it without SHIFT. Now we accept both variants for help keybinding.\n\nFixes help overlay not appearing when pressing '?' on Windows.",
          "timestamp": "2026-02-08T13:33:31+08:00",
          "tree_id": "977cbe946e218e86c9868a42be602e1575706bbb",
          "url": "https://github.com/MasuRii/RustDupe/commit/28ff5c65655845acb66f4ca373df679bcf209446"
        },
        "date": 1770529057338,
        "tool": "cargo",
        "benches": [
          {
            "name": "walker_150_files",
            "value": 526508,
            "range": "± 10833",
            "unit": "ns/iter"
          },
          {
            "name": "hasher/blake3_1KB",
            "value": 7742,
            "range": "± 44",
            "unit": "ns/iter"
          },
          {
            "name": "hasher/blake3_1024KB",
            "value": 334417,
            "range": "± 1554",
            "unit": "ns/iter"
          },
          {
            "name": "hasher/blake3_10240KB",
            "value": 3160179,
            "range": "± 76518",
            "unit": "ns/iter"
          },
          {
            "name": "perceptual_hasher/Phash",
            "value": 935582,
            "range": "± 5883",
            "unit": "ns/iter"
          },
          {
            "name": "perceptual_hasher/Dhash",
            "value": 927463,
            "range": "± 2503",
            "unit": "ns/iter"
          },
          {
            "name": "perceptual_hasher/Ahash",
            "value": 926260,
            "range": "± 1807",
            "unit": "ns/iter"
          },
          {
            "name": "pipeline_approx_80_files",
            "value": 1666968,
            "range": "± 64700",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}