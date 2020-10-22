pub mod test_types;

#[cfg(test)]
mod test_round_trip {
    use serde::{de::DeserializeOwned, Serialize};

    use super::test_types::*;
    use std::{
        collections::HashMap,
        fmt::Debug,
        fs,
        io::Write,
        path::Path,
        process::{Command, Stdio},
    };

    #[cfg(target_os = "windows")]
    static TS_NODE_BIN: &str = "./node_modules/.bin/ts-node.cmd";
    #[cfg(not(target_os = "windows"))]
    static TS_NODE_BIN: &str = "./node_modules/.bin/ts-node";

    fn get_test_code(api_path: &Path, test_code: &str) -> String {
        format!(
            "
import * as api from '{}';
const assert = require('assert');
const getStdin = require('get-stdin');

process.on('unhandledRejection', (error: Error) => {{
    console.error(error);
    process.exit(1);
}});

function test(buffer: Buffer): api.Sink {{
    {}
}}

(async function main() {{
    let inBuf = await getStdin.buffer();
    let outBuf = test(inBuf).getUint8Array();
    process.stdout.write(outBuf);
}})();
        ",
            api_path
                .to_string_lossy()
                .replace("\\", "\\\\")
                .replace(".ts", ""),
            test_code
        )
    }

    fn generate_and_run<T>(test_type: T, test_code: &str)
    where
        T: PartialEq + Debug + Serialize + DeserializeOwned,
    {
        let dir = tempfile::tempdir_in("./tests").unwrap();
        let gen_path = dir.path().join("generated.ts");
        let test_path = dir.path().join("test.ts");

        bincode_typescript::from_file("./tests/test_types.rs", &gen_path, true).unwrap();

        let code = get_test_code(&gen_path, test_code);
        fs::write(&test_path, code).unwrap();

        let mut child = Command::new(TS_NODE_BIN)
            .args(&[test_path])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn process");

        let child_stdin = child.stdin.as_mut().expect("failed to get stdin");

        let encoded: Vec<u8> = bincode::serialize(&test_type).unwrap();
        child_stdin.write_all(&encoded).unwrap();

        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());

        let returned_type: T = bincode::deserialize(&output.stdout).unwrap();
        assert_eq!(test_type, returned_type);
    }

    #[test]
    fn unit_enum() {
        generate_and_run(
            UnitEnum::Three,
            "
        let val = api.readUnitEnum(buffer);
        assert.deepStrictEqual(val, api.UnitEnum.Three);

        return api.writeUnitEnum(api.UnitEnum.Three);
        ",
        );
    }

    #[test]
    fn unit_enum_valued() {
        generate_and_run(
            UnitEnumNumbered::Eight,
            "
        let val = api.readUnitEnumNumbered(buffer);
        assert.deepStrictEqual(val, api.UnitEnumNumbered.Eight);
        assert.deepStrictEqual(val, 8);

        return api.writeUnitEnumNumbered(api.UnitEnumNumbered.Eight)
        ",
        );
    }

    #[test]
    fn tuple_struct() {
        generate_and_run(
            TupleStruct(-145, vec![9987, 456]),
            "
        let val = api.readTupleStruct(buffer);
        let expected: api.TupleStruct = [-145, new Uint32Array([9987, 456])];
        assert.deepStrictEqual(val, expected);

        return api.writeTupleStruct(expected);
        ",
        );
    }

    #[test]
    fn named_struct() {
        generate_and_run(
            NamedStruct {
                zero: Some(28),
                one: 1.23,
                two: 128,
                three: "something".to_string(),
            },
            "
        let val = api.readNamedStruct(buffer);
        let expected = { zero: 28, one: 1.23, two: 128, three: 'something' };
        assert.deepStrictEqual(val, expected);

        return api.writeNamedStruct(expected);
        ",
        );
    }

    #[test]
    fn named_enum_variant() {
        generate_and_run(
            SomeEvent::Named {
                length: 34567,
                interval: 0.0001,
            },
            "
        let val = api.readSomeEvent(buffer);
        let expected = api.SomeEvent.Named({ length: BigInt(34567), interval: 0.0001 });
        assert.deepStrictEqual(val, expected);

        return api.writeSomeEvent(expected);
        ",
        );
    }

    #[test]
    fn unnamed_enum_many_numbers() {
        generate_and_run(SomeEvent::UnnamedMultiple(
            1,
            -2,
            3,
            -4,
            5,
            -6,
            1152921504606846976,
            -8,
            9,
            -10,
            11,
            -12,
            false,
        ),
            "
        let val = api.readSomeEvent(buffer);
        let expected = api.SomeEvent.UnnamedMultiple(1, -2, 3, -4, 5, -6, BigInt(1152921504606846976), BigInt(-8), BigInt(9), BigInt(-10), BigInt(11), BigInt(-12), false);
        assert.deepStrictEqual(val, expected);

        return api.writeSomeEvent(expected);
        ",
        );
    }

    #[test]
    fn named_struct_in_enum() {
        generate_and_run(SomeEvent::NamedStruct {
            inner: NamedStruct {
                zero: None,
                one: 1.23,
                two: 128,
                three: "something".to_string(),
            },
        },
            "
        let val = api.readSomeEvent(buffer);
        let expected = api.SomeEvent.NamedStruct({ inner: { zero: undefined, one: 1.23, two: 128, three: 'something' }});
        assert.deepStrictEqual(val, expected);

        return api.writeSomeEvent(expected);
        ",
        );
    }

    #[test]
    fn hashmap() {
        let mut hm = HashMap::new();
        hm.insert("Some".to_string(), UnitEnum::One);
        hm.insert("More".to_string(), UnitEnum::Three);
        let val = SomeEvent::UnnamedHashMap(Some(hm));
        generate_and_run(val,
            "
        let val = api.readSomeEvent(buffer);
        let expected = api.SomeEvent.UnnamedHashMap(new Map([['Some', api.UnitEnum.One], ['More', api.UnitEnum.Three]]));
        assert.deepStrictEqual(val, expected);

        return api.writeSomeEvent(expected);
        ",
        );
    }
}
