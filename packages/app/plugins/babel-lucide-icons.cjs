const fs = require("fs");
const path = require("path");

let iconMap = null;
function getIconMap() {
  if (!iconMap) {
    iconMap = {};
    try {
      const cjsPath = require.resolve("lucide-react-native", {
        paths: [path.resolve(__dirname, ".."), process.cwd()],
      });
      const lucidePath = path.resolve(path.dirname(cjsPath), "../esm/lucide-react-native.mjs");
      const content = fs.readFileSync(lucidePath, "utf8");
      for (const match of content.matchAll(
        /export\s*\{([^}]+)\}\s*from\s*'\.\/icons\/([^']+)\.mjs'/g,
      )) {
        const file = match[2];
        for (const part of match[1].split(",")) {
          const m = part.match(/default\s+as\s+([A-Za-z0-9_]+)/);
          if (m) iconMap[m[1]] = file;
        }
      }
    } catch (e) {
      console.warn("babel-lucide-icons: failed to read lucide icon map", e);
    }
  }
  return iconMap;
}

module.exports = function ({ types: t }) {
  return {
    name: "babel-lucide-icons",
    visitor: {
      ImportDeclaration(path) {
        if (path.node.source.value !== "lucide-react-native") return;
        if (path.node.importKind === "type") {
          path.remove();
          return;
        }

        const map = getIconMap();
        if (!map || Object.keys(map).length === 0) return;

        const newImports = [];
        const remainingSpecifiers = [];

        for (const specifier of path.node.specifiers) {
          if (t.isImportSpecifier(specifier)) {
            const importedName = specifier.imported.name || specifier.imported.value;
            if (specifier.importKind !== "type") {
              const kebab = map[importedName];
              if (kebab) {
                newImports.push(
                  t.importDeclaration(
                    [t.importDefaultSpecifier(specifier.local)],
                    t.stringLiteral(`lucide-react-native/icons/${kebab}`),
                  ),
                );
                continue;
              }
            }
          }
          remainingSpecifiers.push(specifier);
        }

        const valueSpecifiers = remainingSpecifiers.filter(
          (s) =>
            s.importKind !== "type" &&
            s.imported?.name !== "LucideIcon" &&
            s.imported?.name !== "LucideProps",
        );

        if (newImports.length > 0) {
          if (valueSpecifiers.length > 0) {
            path.node.specifiers = valueSpecifiers;
            path.insertAfter(newImports);
          } else {
            path.replaceWithMultiple(newImports);
          }
        } else if (remainingSpecifiers.length > 0 && valueSpecifiers.length === 0) {
          path.remove();
        }
      },
    },
  };
};
