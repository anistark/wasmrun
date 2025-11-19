//! Control flow analysis for WASM bytecode
//!
//! Analyzes bytecode structure to identify block boundaries and label positions
//! This is necessary for implementing proper branch semantics

use std::collections::HashMap;

/// Result of control flow analysis
#[derive(Debug, Clone)]
pub struct ControlFlowInfo {
    /// Map of bytecode position to block end position
    /// Used to jump to end when br/br_if target this block
    pub block_ends: HashMap<usize, usize>,
    /// Map of bytecode position to loop start position
    /// Used to jump back when br/br_if target this loop
    pub loop_starts: HashMap<usize, usize>,
}

/// Analyze bytecode structure for control flow
pub fn analyze_control_flow(bytecode: &[u8]) -> Result<ControlFlowInfo, String> {
    let mut block_ends = HashMap::new();
    let mut loop_starts = HashMap::new();
    let mut block_stack = Vec::new();

    let mut pos = 0;
    while pos < bytecode.len() {
        let byte = bytecode[pos];

        match byte {
            0x02 => {
                // block instruction
                block_stack.push((pos, None)); // (start_pos, loop_flag)
                pos += 1;
                // Skip block type
                skip_block_type(bytecode, &mut pos);
            }
            0x03 => {
                // loop instruction
                block_stack.push((pos, Some(pos))); // mark as loop with start position
                pos += 1;
                // Skip block type
                skip_block_type(bytecode, &mut pos);
            }
            0x04 => {
                // if instruction
                block_stack.push((pos, None));
                pos += 1;
                // Skip block type
                skip_block_type(bytecode, &mut pos);
            }
            0x0B => {
                // end instruction - closes current block
                if let Some((start_pos, loop_flag)) = block_stack.pop() {
                    if let Some(_loop_start) = loop_flag {
                        // This is a loop - store loop start position
                        loop_starts.insert(start_pos, start_pos);
                    } else {
                        // This is a block or if - store end position
                        block_ends.insert(start_pos, pos + 1);
                    }
                }
                pos += 1;
            }
            0x05 => {
                // else instruction - still within a block
                pos += 1;
            }
            _ => {
                // Skip over other instructions
                pos += skip_instruction_bytes(byte);
            }
        }
    }

    Ok(ControlFlowInfo {
        block_ends,
        loop_starts,
    })
}

/// Skip block type in bytecode
fn skip_block_type(bytecode: &[u8], pos: &mut usize) {
    if *pos < bytecode.len() {
        *pos += 1; // All block types are 1 byte in this simplified version
    }
}

/// Calculate how many bytes to skip for a given instruction
fn skip_instruction_bytes(byte: u8) -> usize {
    match byte {
        // Constants with LEB128 immediates
        0x41..=0x42 => {
            // i32/i64.const - skip the immediate
            // This is approximate - LEB128 can be variable length
            1
        }
        0x43..=0x44 => {
            // f32/f64.const - skip 4/8 byte immediate
            1
        }
        // Local operations
        0x20..=0x22 => {
            // local.get/set/tee - skip local index
            1
        }
        // Global operations
        0x23..=0x24 => {
            // global.get/set - skip global index
            1
        }
        // Memory operations with memarg
        0x28..=0x3c => {
            // Memory ops - skip align and offset
            1
        }
        0x0c..=0x0d => {
            // br, br_if - skip label index
            1
        }
        0x0e => {
            // br_table - more complex, skip for now
            1
        }
        0x10..=0x11 => {
            // call, call_indirect - skip function/type index
            1
        }
        _ => {
            // Most other instructions are 1 byte
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_empty_block() {
        // block (0x02), void type (0x40), nop (0x01), end (0x0b)
        let bytecode = vec![0x02, 0x40, 0x01, 0x0b];
        let info = analyze_control_flow(&bytecode).unwrap();
        assert!(info.block_ends.contains_key(&0));
    }

    #[test]
    fn test_analyze_loop() {
        // loop (0x03), void type (0x40), i32.const 0 (0x41, 0x00), end (0x0b)
        let bytecode = vec![0x03, 0x40, 0x41, 0x00, 0x0b];
        let info = analyze_control_flow(&bytecode).unwrap();
        assert!(info.loop_starts.contains_key(&0));
    }

    #[test]
    fn test_analyze_nested_blocks() {
        // block, block, end, end
        let bytecode = vec![0x02, 0x40, 0x02, 0x40, 0x0b, 0x0b];
        let info = analyze_control_flow(&bytecode).unwrap();
        assert_eq!(info.block_ends.len(), 2);
    }
}
