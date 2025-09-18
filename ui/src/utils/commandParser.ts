export interface ParsedCommand {
  type: 'function' | 'help' | 'memory' | 'clear' | 'list' | 'unknown'
  name?: string
  args?: any[]
  raw: string
}

export function parseCommand(input: string): ParsedCommand {
  const trimmed = input.trim()

  // Handle special commands
  if (trimmed === 'help') {
    return { type: 'help', raw: input }
  }

  if (trimmed === 'clear') {
    return { type: 'clear', raw: input }
  }

  if (trimmed === 'list' || trimmed === 'ls') {
    return { type: 'list', raw: input }
  }

  if (trimmed.startsWith('memory.')) {
    return { type: 'memory', name: trimmed, raw: input }
  }

  // Parse function calls: functionName(arg1, arg2, ...)
  const functionMatch = trimmed.match(/^(\w+)\s*\(\s*(.*?)\s*\)$/)
  if (functionMatch) {
    const [, name, argsStr] = functionMatch
    const args = parseArguments(argsStr)
    return { type: 'function', name, args, raw: input }
  }

  // Parse function calls without parentheses: functionName
  const simpleMatch = trimmed.match(/^(\w+)$/)
  if (simpleMatch) {
    const [, name] = simpleMatch
    return { type: 'function', name, args: [], raw: input }
  }

  return { type: 'unknown', raw: input }
}

function parseArguments(argsStr: string): any[] {
  if (!argsStr.trim()) return []

  const args: any[] = []
  let current = ''
  let inString = false
  let stringChar = ''
  let bracketDepth = 0
  let parenDepth = 0

  for (let i = 0; i < argsStr.length; i++) {
    const char = argsStr[i]

    if (!inString) {
      if (char === '"' || char === "'") {
        inString = true
        stringChar = char
        current += char
      } else if (char === '[') {
        bracketDepth++
        current += char
      } else if (char === ']') {
        bracketDepth--
        current += char
      } else if (char === '(') {
        parenDepth++
        current += char
      } else if (char === ')') {
        parenDepth--
        current += char
      } else if (char === ',' && bracketDepth === 0 && parenDepth === 0) {
        args.push(parseValue(current.trim()))
        current = ''
      } else {
        current += char
      }
    } else {
      current += char
      if (char === stringChar && argsStr[i - 1] !== '\\') {
        inString = false
        stringChar = ''
      }
    }
  }

  if (current.trim()) {
    args.push(parseValue(current.trim()))
  }

  return args
}

function parseValue(value: string): any {
  const trimmed = value.trim()

  // String values (quoted)
  if (
    (trimmed.startsWith('"') && trimmed.endsWith('"')) ||
    (trimmed.startsWith("'") && trimmed.endsWith("'"))
  ) {
    return trimmed.slice(1, -1)
  }

  // Array values
  if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
    try {
      return JSON.parse(trimmed)
    } catch {
      // Fallback: parse as comma-separated numbers
      const content = trimmed.slice(1, -1)
      return content.split(',').map(v => parseValue(v.trim()))
    }
  }

  // Boolean values
  if (trimmed === 'true') return true
  if (trimmed === 'false') return false

  // Null/undefined
  if (trimmed === 'null') return null
  if (trimmed === 'undefined') return undefined

  // Numeric values
  if (/^-?\d+$/.test(trimmed)) {
    return parseInt(trimmed, 10)
  }

  if (/^-?\d*\.?\d+$/.test(trimmed)) {
    return parseFloat(trimmed)
  }

  // Default: return as string (unquoted)
  return trimmed
}

export function getCommandSuggestions(input: string, availableFunctions: string[]): string[] {
  const trimmed = input.trim().toLowerCase()

  const suggestions: string[] = []

  // Built-in commands
  const builtinCommands = ['help', 'clear', 'list', 'memory.size()', 'memory.grow(1)']
  suggestions.push(...builtinCommands.filter(cmd => cmd.toLowerCase().startsWith(trimmed)))

  // Available functions
  suggestions.push(
    ...availableFunctions.filter(fn => fn.toLowerCase().startsWith(trimmed)).map(fn => `${fn}()`)
  )

  return suggestions.slice(0, 10) // Limit to 10 suggestions
}
