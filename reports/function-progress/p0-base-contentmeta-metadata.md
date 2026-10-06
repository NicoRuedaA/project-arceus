> **SUPERSEDED by p0-base-main-pin-reconciliation.md (2026-10-06).**
> The base `main` SHA-256 identity pin is reconciled: the canonical artifact
> `work/pla/pk1/exefs/main` matches the only documented pin (SHA-256 `6f0e5f4a…`). The
> `Program`→NCZ→ExeFS `s02` chain and the six base ExeFS members are later qualified
> with 15/15 NSO segment hashes verified (`p0-base-v0-module-provenance.md`). The
> `KeyError` NCA/ExeFS-open limitation below is retained as historical. Historical
> content is preserved below.

# Metadatos de ContentMeta del paquete base

## Resultado

La inspección de nivel superior confirma que el contenedor PFS0 enumera seis
miembros: uno NCZ, cuatro NCA —incluido el NCA de CNMT— y un XML de CNMT.
El XML declara un ContentMeta de tipo `Application`, versión `0`, y cinco
registros de contenido, incluido un registro `Program`. El identificador del
registro `Program` coincide con el nombre-base de un miembro enumerado. En
total, cuatro de los cinco identificadores coinciden con nombres-base; el
registro `Meta` no coincide.

## Alcance y límite

La evidencia se limita al directorio PFS0 y al XML de CNMT de nivel superior.
La comprobación de identidad volvió a derivar desde PFS0/CNMT el NCZ asociado
al registro `Program`. Tras verificar al menos 10 GiB libres en un volumen de
scratch externo al repositorio, se copió únicamente ese NCZ a un directorio
temporal y se descomprimió a un NCA. La apertura de NCA/ExeFS falló con
`KeyError` al solicitar material de clave requerido: la API/configuración
actual no resolvió esa solicitud. Esto no demuestra que el material de clave
esté ausente; el requisito exacto sigue siendo desconocido.

No se leyeron bytes de `main`/NSO ni se calculó su SHA. No se realizó parsing
semántico ni descompilación. El directorio temporal y sus archivos se
eliminaron; el repositorio no cambió.

Esto **no reconcilia el pin SHA registrado del NSO base-main**, no identifica
los bytes NSO exactos y no autoriza analizar el candidato cuyo pin discrepa.
El estado P0 permanece en curso: la identidad/pin sigue sin reconciliarse y el
candidato no está autorizado para análisis. Siguiente paso: resolver la
configuración local de API/acceso al NCA sin registrar claves; repetir solo la
comparación de identidad SHA y analizar el candidato únicamente si coincide
exactamente con el pin registrado. Lo no demostrado sigue desconocido; no se
afirma completitud.
