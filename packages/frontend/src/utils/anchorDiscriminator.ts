import { sha256 } from '@noble/hashes/sha256'
import { utf8ToBytes } from '@noble/hashes/utils'

/** Anchor discriminator: sha256("global:<method_name>")[0..8] */
export function anchorDiscriminator(methodName: string): Buffer {
  const hash = sha256(utf8ToBytes(`global:${methodName}`))
  return Buffer.from(hash.slice(0, 8))
}

/** Anchor account discriminator: sha256("account:<RustStructName>")[0..8] */
export function anchorAccountDiscriminator(accountStructName: string): Buffer {
  const hash = sha256(utf8ToBytes(`account:${accountStructName}`))
  return Buffer.from(hash.slice(0, 8))
}
