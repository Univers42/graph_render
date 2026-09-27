/* ************************************************************************** */
/*                                                                            */
/*                                                        :::      ::::::::   */
/*   ts-extension-loader.mjs                            :+:      :+:    :+:   */
/*                                                    +:+ +:+         +:+     */
/*   By: dlesieur <dlesieur@student.42.fr>          +#+  :+:       +#+        */
/*                                                +#+#+#+#+#+   +#+           */
/*   Created: 2026/09/27 00:00:00 by dlesieur          #+#    #+#             */
/*   Updated: 2026/09/27 00:00:00 by dlesieur         ###   ########.fr       */
/*                                                                            */
/* ************************************************************************** */

// Node ESM loader: resolve extensionless relative specifiers to .ts/.tsx.
//
// Why this exists: `src/` is written in the bundler resolution style — imports
// like "./core/math" carry no extension, which Vite and tsc
// (moduleResolution: "bundler") accept. Node's ESM resolver does not: it wants
// an explicit filename, so a bare `node --test` dies with
// ERR_MODULE_NOT_FOUND on the first relative import. This loader bridges the
// two so the suite runs on plain Node with no bundler and no vitest.
//
// Ported from osionos's tests/canvas/ts-extension-loader.mjs, keeping only the
// reusable half. That loader also mirrored the host's Vite/tsconfig aliases
// ("@/" -> src, "@osionos/*" -> packages/*) for app files tested outside Vite;
// none of those aliases exist in this standalone repo, so carrying them over
// would only add dead references to directories that are not here.
//
// This is a resolution shim, not a type checker or a transpiler: type stripping
// is Node's own --experimental-strip-types. It does not rewrite anything on disk.

function hasKnownExtension(specifier) {
  return [".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs", ".json"].some((extension) =>
    specifier.endsWith(extension),
  );
}

export async function resolve(specifier, context, nextResolve) {
  const isRelative = specifier.startsWith(".") || specifier.startsWith("file:");
  if (!isRelative || hasKnownExtension(specifier)) {
    return nextResolve(specifier, context);
  }

  // A bare specifier is a .ts/.tsx file, else a directory with an index
  // (barrel imports like "../src/core/model").
  let lastError;
  for (const suffix of [".ts", ".tsx", "/index.ts", "/index.tsx"]) {
    try {
      return await nextResolve(`${specifier}${suffix}`, context);
    } catch (error) {
      // Only a "not found" is a real miss. Anything else (a syntax error inside
      // a candidate, a bad export) is the real failure and must not be masked
      // by trying the next suffix.
      if (error?.code !== "ERR_MODULE_NOT_FOUND") throw error;
      lastError = error;
    }
  }
  throw lastError;
}
