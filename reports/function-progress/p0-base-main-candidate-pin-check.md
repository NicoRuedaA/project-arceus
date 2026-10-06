# Comprobación del pin del candidato local base-main

## Resultado

El candidato local coincide con el tamaño registrado (31.755.066 bytes) y con el
ID de módulo de `base_main_identity`, pero no coincide con el SHA-256 fijado en el
registro. La comprobación terminó con código 2 en la puerta de identidad. No se
validaron segmentos NSO y no se ejecutó ningún parser dinámico.

Esto solo invalida la confianza en que este candidato local corresponda al hash
registrado. No invalida el ID/tamaño ni el inventario de funciones/aristas de llamada
independientemente censados en [la fila de identidad base-main](../../sheets/re/p0_scope_census.tsv#base_main_identity).
La procedencia del paquete base y la relación de este candidato con él continúan
desconocidas. El informe anterior de Suyu registra el pin base-main usado como
control de exclusión ([comprobaciones locales](../suyu/update-v262144-feasibility.md#local-checks-with-bounded-scope)); no se publica aquí la ruta local ni el hash observado en esta nueva comprobación.

## Método y límite

Se compararon únicamente los campos de tamaño, ID de módulo y pin SHA-256 antes
de cualquier lectura estructural posterior. El tamaño y el ID coincidieron; el
pin SHA-256 no. Por tanto, no hay evidencia de validación de segmentos ni de
identidad exacta del candidato, y ningún resultado del parser puede atribuirse a
esta comprobación.

## Siguiente acción

Reconciliar el origen y la identidad del candidato contra el pin registrado, u
obtener de nuevo la entrada exacta que satisface ese pin. No parsear este candidato
hasta superar esa puerta. Mantener separados este resultado y el inventario
histórico de funciones base-main.
