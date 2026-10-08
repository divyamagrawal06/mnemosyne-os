;; Original MnemosyneOS replay fixture. All persistent guest state is the single
;; fixed memory and exported mutable global. There are no tables or threads.
(module
  (import "env" "clock" (func $clock (result i64)))
  (import "env" "random" (func $random (param i32) (result i64)))
  (import "env" "emit" (func $emit (param i32 i64 i64)))
  (memory (export "memory") 1 1)
  (global $version (export "version") (mut i64) (i64.const 0))

  (func (export "apply") (param $key i32) (param $delta i64) (result i64)
    (local $value i64)
    (if (i32.ge_u (local.get $key) (i32.const 16)) (then unreachable))
    (local.set $value
      (i64.add
        (i64.add
          (i64.load (i32.mul (local.get $key) (i32.const 8)))
          (local.get $delta))
        (i64.add
          (i64.and (call $clock) (i64.const 31))
          (call $random (i32.const 97)))))
    (i64.store (i32.mul (local.get $key) (i32.const 8)) (local.get $value))
    (global.set $version (i64.add (global.get $version) (i64.const 1)))
    (call $emit (local.get $key) (local.get $value) (global.get $version))
    (local.get $value)))
