import com.android.build.api.variant.AndroidComponentsExtension
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.Plugin
import org.gradle.api.Project
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.TaskAction
import java.io.File

const val RUST_TASK_GROUP = "rust"

open class Config {
    var rootDirRel: String? = null
    var targets: List<String>? = null
    var arches: List<String>? = null
}

open class RustPlugin : Plugin<Project> {
    override fun apply(project: Project) {
        val config = project.extensions.create("rust", Config::class.java)

        project.afterEvaluate {
            val androidComponents =
                project.extensions.findByType(AndroidComponentsExtension::class.java)
                    ?: throw GradleException("Android plugin not found")

            val targets = config.targets ?: listOf("aarch64")
            val arches = config.arches ?: listOf("arm64-v8a")

            androidComponents.onVariants { variant ->
                val variantName =
                    variant.name.replaceFirstChar { it.uppercase() }

                val buildTask =
                    project.tasks.maybeCreate(
                        "rustBuild${variantName}",
                        RustBuildTask::class.java,
                    ).apply {
                        group = RUST_TASK_GROUP
                        description = "Build Rust library for $variantName"
                        rootDirRel = config.rootDirRel ?: "../../../"
                        this.targets = targets
                        this.arches = arches
                        this.profile = if (variant.name.contains("release", ignoreCase = true)) "release" else "dev"
                    }

                val jniLibsDir =
                    File(project.projectDir, "src/main/jniLibs")

                project.tasks.named("merge${variantName}JniLibFolders").configure {
                    dependsOn(buildTask)
                }
            }
        }
    }
}

open class RustBuildTask : DefaultTask() {
    @Input
    var rootDirRel: String = "../../../"

    @Input
    var targets: List<String> = listOf("aarch64")

    @Input
    var arches: List<String> = listOf("arm64-v8a")

    @Input
    var profile: String = "dev"

    @TaskAction
    fun build() {
        val rootDir = File(project.projectDir, rootDirRel).canonicalFile

        val targetTriples = mapOf(
            "aarch64" to "aarch64-linux-android",
            "armv7" to "armv7-linux-androideabi",
            "i686" to "i686-linux-android",
            "x86_64" to "x86_64-linux-android",
        )

        val profileFlag = if (profile == "release") "--release" else ""

        for ((i, target) in targets.withIndex()) {
            val triple = targetTriples[target]
                ?: throw GradleException("Unknown target: $target")
            val arch = arches[i]

            project.exec {
                workingDir = rootDir
                commandLine(
                    "cargo", "build",
                    "-p", "nsynergy-tauri",
                    "--target", triple,
                    *(if (profileFlag.isNotEmpty()) arrayOf(profileFlag) else emptyArray())
                )
            }

            val profileDir = if (profile == "release") "release" else "debug"
            val libSrc = File(rootDir, "target/$triple/$profileDir/libnsynergy_tauri_lib.so")
            val jniDir = File(project.projectDir, "src/main/jniLibs/$arch")
            jniDir.mkdirs()

            if (libSrc.exists()) {
                libSrc.copyTo(File(jniDir, "libnsynergy_tauri_lib.so"), overwrite = true)
            }
        }
    }
}
